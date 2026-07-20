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
}

/// One entry of `GET /api/timeline/buckets` (Immich v3): a month + its count.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TimeBucketDto {
    time_bucket: String,
}

/// `GET /api/timeline/bucket` returns columnar data (struct-of-arrays), not a
/// list of asset objects. We only need the id column; full metadata comes from
/// the per-asset endpoint.
#[derive(Deserialize)]
struct BucketAssetsDto {
    #[serde(default)]
    id: Vec<String>,
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

/// EXIF orientations 5-8 are 90°/270° rotations: Immich reports the SENSOR
/// dimensions but renders previews rotated, so width/height must be swapped
/// (classic on iPhone HEICs, orientation "6"). Immich stores the value as a
/// string; accept a bare number too.
pub(crate) fn is_rotated_orientation(e: &Option<serde_json::Value>) -> bool {
    let o = match e.as_ref().and_then(|v| v.get("orientation")) {
        Some(v) => v,
        None => return false,
    };
    let code = o.as_i64().or_else(|| o.as_str().and_then(|s| s.trim().parse::<i64>().ok()));
    matches!(code, Some(5..=8))
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

/// Immich answers 401 both for a revoked/unknown share key and for a
/// password-protected link we hold no token for. Only the body tells them apart,
/// and the distinction matters: a revoked key must surface as `NotFound` so the
/// tenant is delisted as dead, while a locked one must surface as
/// `PasswordRequired` so visitors get the password prompt instead.
pub(crate) fn classify_unauthorized(body: &str) -> AppError {
    if body.contains("Invalid share key") || body.contains("Invalid share slug") {
        AppError::NotFound
    } else {
        AppError::PasswordRequired
    }
}

/// Normalize an Immich asset DTO into the record we cache locally.
fn map_asset(a: AssetDto) -> Asset {
    let w = exif_i64(&a.exif_info, "exifImageWidth").unwrap_or(0);
    let h = exif_i64(&a.exif_info, "exifImageHeight").unwrap_or(0);
    let (mut width, mut height) = if w > 0 && h > 0 { (w, h) } else { (3, 2) };
    if is_rotated_orientation(&a.exif_info) {
        (width, height) = (height, width);
    }

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
}

impl ImmichClient {
    pub fn new(base: &str) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            // Idle/read timeout so a stalled upstream stream errors instead of
            // hanging forever. Not a total-duration cap: healthy large downloads
            // keep making progress and are unaffected.
            .read_timeout(std::time::Duration::from_secs(30))
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
            return Err(classify_unauthorized(&body));
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

    /// Issue an authenticated read-only GET against Immich and decode JSON.
    /// `what` names the call for error messages.
    async fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        url: String,
        key: &str,
        token: Option<&str>,
        what: &str,
    ) -> AppResult<T> {
        let mut req = self
            .http
            .get(url)
            .headers(Self::key_headers(key)?)
            .timeout(std::time::Duration::from_secs(60));
        if let Some((n, v)) = Self::cookie_header(token) {
            req = req.header(n, v);
        }
        let resp = req.send().await.map_err(|e| AppError::Upstream(e.to_string()))?;

        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            let body = resp.text().await.unwrap_or_default();
            return Err(classify_unauthorized(&body));
        }
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(AppError::NotFound);
        }
        if !status.is_success() {
            return Err(AppError::Upstream(format!("{what} {status}")));
        }
        resp.json()
            .await
            .map_err(|e| AppError::Upstream(format!("decode {what}: {e}")))
    }

    /// Fetch the album's complete asset id list via the share key.
    ///
    /// Immich v3 removed the inline `assets` array from `GET /api/albums/{id}`
    /// (it still returns 200 with `assetCount`, just no assets — a silent break),
    /// so the listing now goes through the timeline API: one call for the month
    /// buckets, then one per bucket. Both accept a share key. Metadata is NOT
    /// available here — only ids — so callers pair this with `asset_detail`.
    pub async fn album_asset_ids(
        &self,
        album_id: &str,
        key: &str,
        token: Option<&str>,
    ) -> AppResult<Vec<String>> {
        let buckets: Vec<TimeBucketDto> = self
            .get_json(
                format!(
                    "{}/api/timeline/buckets?albumId={}&key={}",
                    self.base,
                    enc(album_id),
                    enc(key)
                ),
                key,
                token,
                "timeline/buckets",
            )
            .await?;

        // Sequential: buckets are one-per-month and cheap, and a sync is rare.
        let mut ids = Vec::new();
        for b in &buckets {
            let page: BucketAssetsDto = self
                .get_json(
                    format!(
                        "{}/api/timeline/bucket?albumId={}&timeBucket={}&key={}",
                        self.base,
                        enc(album_id),
                        enc(&b.time_bucket),
                        enc(key)
                    ),
                    key,
                    token,
                    "timeline/bucket",
                )
                .await?;
            ids.extend(page.id);
        }
        Ok(ids)
    }

    /// Fetch one asset's full metadata (filename, exif, tags, dimensions).
    /// The timeline API only yields ids, so this fills in everything the local
    /// cache stores. Callers should only invoke it for assets not already cached.
    pub async fn asset_detail(
        &self,
        asset_id: &str,
        key: &str,
        token: Option<&str>,
    ) -> AppResult<Asset> {
        let dto: AssetDto = self
            .get_json(
                format!("{}/api/assets/{}?key={}", self.base, enc(asset_id), enc(key)),
                key,
                token,
                "assets/{id}",
            )
            .await?;
        Ok(map_asset(dto))
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
