use reqwest::header::{HeaderMap, HeaderValue, COOKIE, RANGE, SET_COOKIE};
use serde::Deserialize;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::error::{AppError, AppResult};

/// Read-only client for the Immich API. Authenticates with a public share key.
/// This client only ever issues GET requests; it never mutates Immich.
#[derive(Clone)]
pub struct ImmichClient {
    base: String,
    http: reqwest::Client,
}

/// Normalized asset record we cache locally.
#[derive(Debug, Clone)]
pub struct Asset {
    pub id: String,
    pub kind: String, // IMAGE | VIDEO
    pub width: i64,
    pub height: i64,
    pub taken_at: i64,
    pub filename: String,
    pub tags: String,  // denormalized metadata text for search
    pub exif: String,  // full exifInfo as JSON (for the metadata panel)
}

/// Result of validating a share key.
#[derive(Debug, Clone)]
pub struct ShareInfo {
    pub album_id: String,
    pub title: String,
    pub allow_download: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SharedLinkDto {
    #[serde(default)]
    album: Option<AlbumDto>,
    #[serde(default)]
    allow_download: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AlbumDto {
    id: String,
    #[serde(default)]
    album_name: String,
    #[serde(default)]
    assets: Vec<AssetDto>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AssetDto {
    id: String,
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    original_file_name: String,
    #[serde(default)]
    file_created_at: Option<String>,
    #[serde(default)]
    local_date_time: Option<String>,
    // Whole exifInfo object, captured raw so the metadata panel can show everything.
    #[serde(default)]
    exif_info: Option<serde_json::Value>,
}

fn exif_i64(e: &Option<serde_json::Value>, key: &str) -> Option<i64> {
    e.as_ref()?.get(key)?.as_i64()
}
fn exif_str(e: &Option<serde_json::Value>, key: &str) -> Option<String> {
    e.as_ref()?
        .get(key)?
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

fn parse_ts(s: &Option<String>) -> i64 {
    s.as_deref()
        .and_then(|v| OffsetDateTime::parse(v, &Rfc3339).ok())
        .map(|dt| dt.unix_timestamp())
        .unwrap_or(0)
}

impl ImmichClient {
    pub fn new(base: &str) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .pool_max_idle_per_host(16)
            .build()
            .expect("failed to build reqwest client");
        ImmichClient {
            base: base.to_string(),
            http,
        }
    }

    fn key_headers(key: &str) -> AppResult<HeaderMap> {
        let mut h = HeaderMap::new();
        let hv = HeaderValue::from_str(key)
            .map_err(|_| AppError::BadRequest("invalid share key".into()))?;
        h.insert("x-immich-share-key", hv);
        Ok(h)
    }

    fn cookie_header(token: Option<&str>) -> Option<(reqwest::header::HeaderName, HeaderValue)> {
        let t = token?;
        let hv = HeaderValue::from_str(&format!("immich_shared_link_token={t}")).ok()?;
        Some((COOKIE, hv))
    }

    /// Validate a share key and return the album id + metadata.
    /// `token` is the immich_shared_link_token for password-protected links.
    /// Returns `PasswordRequired` if the link needs a password we don't have.
    pub async fn share_info(&self, key: &str, token: Option<&str>) -> AppResult<ShareInfo> {
        let url = format!("{}/api/shared-links/me?key={}", self.base, enc(key));
        let mut req = self
            .http
            .get(url)
            .headers(Self::key_headers(key)?)
            .timeout(std::time::Duration::from_secs(20));
        if let Some((n, v)) = Self::cookie_header(token) {
            req = req.header(n, v);
        }
        let resp = req.send().await.map_err(|e| AppError::Upstream(e.to_string()))?;

        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            // Distinguish a bad key from a password-protected link (IPP's heuristic).
            let body = resp.text().await.unwrap_or_default();
            if body.contains("Invalid share key") || body.contains("Invalid share slug") {
                return Err(AppError::NotFound);
            }
            return Err(AppError::PasswordRequired);
        }
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(AppError::NotFound);
        }
        if !status.is_success() {
            return Err(AppError::Upstream(format!("shared-links/me {status}")));
        }

        let dto: SharedLinkDto = resp
            .json()
            .await
            .map_err(|e| AppError::Upstream(format!("decode shared link: {e}")))?;

        let album = dto.album.ok_or(AppError::NotFound)?;
        Ok(ShareInfo {
            album_id: album.id,
            title: album.album_name,
            allow_download: dto.allow_download,
        })
    }

    /// Exchange a password for an immich_shared_link_token (cookie). Returns the token.
    /// `Forbidden` means the password was wrong.
    pub async fn login(&self, key: &str, password: &str) -> AppResult<String> {
        let url = format!("{}/api/shared-links/login?key={}", self.base, enc(key));
        let resp = self
            .http
            .post(url)
            .headers(Self::key_headers(key)?)
            .json(&serde_json::json!({ "password": password }))
            .timeout(std::time::Duration::from_secs(20))
            .send()
            .await
            .map_err(|e| AppError::Upstream(e.to_string()))?;

        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED
            || status == reqwest::StatusCode::FORBIDDEN
            || status == reqwest::StatusCode::BAD_REQUEST
        {
            return Err(AppError::Forbidden); // wrong password
        }
        if !status.is_success() {
            return Err(AppError::Upstream(format!("shared-links/login {status}")));
        }

        for v in resp.headers().get_all(SET_COOKIE).iter() {
            if let Ok(s) = v.to_str() {
                if let Some(rest) = s.split("immich_shared_link_token=").nth(1) {
                    let token = rest.split(';').next().unwrap_or("").trim().to_string();
                    if !token.is_empty() {
                        return Ok(token);
                    }
                }
            }
        }
        Err(AppError::Upstream("login returned no token".into()))
    }

    /// Fetch the full asset list for an album via the share key.
    pub async fn album_assets(
        &self,
        album_id: &str,
        key: &str,
        token: Option<&str>,
    ) -> AppResult<Vec<Asset>> {
        let url = format!(
            "{}/api/albums/{}?key={}&withoutAssets=false",
            self.base,
            enc(album_id),
            enc(key)
        );
        let mut req = self
            .http
            .get(url)
            .headers(Self::key_headers(key)?)
            .timeout(std::time::Duration::from_secs(60));
        if let Some((n, v)) = Self::cookie_header(token) {
            req = req.header(n, v);
        }
        let resp = req.send().await.map_err(|e| AppError::Upstream(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(AppError::Upstream(format!("albums/{album_id} {}", resp.status())));
        }

        let album: AlbumDto = resp
            .json()
            .await
            .map_err(|e| AppError::Upstream(format!("decode album: {e}")))?;

        let assets = album
            .assets
            .into_iter()
            .map(|a| {
                let w = exif_i64(&a.exif_info, "exifImageWidth").unwrap_or(0);
                let h = exif_i64(&a.exif_info, "exifImageHeight").unwrap_or(0);
                let (width, height) = if w > 0 && h > 0 { (w, h) } else { (3, 2) };

                let mut taken = parse_ts(&a.file_created_at);
                if taken == 0 {
                    taken = parse_ts(&a.local_date_time);
                }

                // Searchable text: meaningful exif VALUES (not raw JSON keys) +
                // camera/lens so queries like "Canon", "iPhone", a lens, a city, or a
                // caption word all match. The full exif blob is stored separately.
                let tags = [
                    "description",
                    "city",
                    "state",
                    "country",
                    "make",
                    "model",
                    "lensModel",
                ]
                .iter()
                .filter_map(|k| exif_str(&a.exif_info, k))
                .collect::<Vec<_>>()
                .join(" ");

                let exif = a
                    .exif_info
                    .as_ref()
                    .map(|v| v.to_string())
                    .unwrap_or_default();

                Asset {
                    id: a.id,
                    kind: if a.kind.is_empty() { "IMAGE".into() } else { a.kind },
                    width,
                    height,
                    taken_at: taken,
                    filename: a.original_file_name,
                    tags,
                    exif,
                }
            })
            .collect();

        Ok(assets)
    }

    /// Stream an asset's bytes from Immich. `size` is thumbnail | preview | original.
    /// Returns the raw upstream response so the handler can relay status/headers/body.
    pub async fn fetch_bytes(
        &self,
        asset_id: &str,
        size: &str,
        key: &str,
        token: Option<&str>,
        range: Option<&str>,
    ) -> AppResult<reqwest::Response> {
        let url = match size {
            "thumbnail" => format!(
                "{}/api/assets/{}/thumbnail?size=thumbnail&key={}",
                self.base,
                enc(asset_id),
                enc(key)
            ),
            "preview" => format!(
                "{}/api/assets/{}/thumbnail?size=preview&key={}",
                self.base,
                enc(asset_id),
                enc(key)
            ),
            "original" => format!(
                "{}/api/assets/{}/original?key={}",
                self.base,
                enc(asset_id),
                enc(key)
            ),
            _ => return Err(AppError::BadRequest("invalid size".into())),
        };

        let mut req = self.http.get(url).headers(Self::key_headers(key)?);
        if let Some((n, v)) = Self::cookie_header(token) {
            req = req.header(n, v);
        }
        if let Some(r) = range {
            if let Ok(hv) = HeaderValue::from_str(r) {
                req = req.header(RANGE, hv);
            }
        }
        let resp = req.send().await.map_err(|e| AppError::Upstream(e.to_string()))?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(AppError::NotFound);
        }
        if !resp.status().is_success() && resp.status() != reqwest::StatusCode::PARTIAL_CONTENT {
            return Err(AppError::Upstream(format!("asset bytes {}", resp.status())));
        }
        Ok(resp)
    }
}

/// Percent-encode a path/query component.
fn enc(s: &str) -> String {
    use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
    utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
}
