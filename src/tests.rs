//! Unit tests for pure / deterministic backend logic.
//! Wired in from main.rs via `#[cfg(test)] mod tests;`.

use std::time::Duration;

use crate::config::{parse_sync_ttl, Config};
use crate::ratelimit::RateLimiter;
use crate::routes::download::safe_name;
use crate::visitor::{make_cookie_value, sign, unlock_set_cookie, verify};

const SECRET: &[u8] = b"unit-test-cookie-secret-0123456789";

// ---------------------------------------------------------------------------
// config: SYNC_TTL_SECS clamp
// ---------------------------------------------------------------------------

#[test]
fn sync_ttl_valid_value_honored() {
    assert_eq!(parse_sync_ttl("7200"), 7200);
    assert_eq!(parse_sync_ttl("1"), 1);
}

#[test]
fn sync_ttl_zero_falls_back_to_default() {
    assert_eq!(parse_sync_ttl("0"), 3600);
}

#[test]
fn sync_ttl_negative_falls_back_to_default() {
    assert_eq!(parse_sync_ttl("-1"), 3600);
    assert_eq!(parse_sync_ttl("-86400"), 3600);
}

#[test]
fn sync_ttl_garbage_falls_back_to_default() {
    assert_eq!(parse_sync_ttl(""), 3600);
    assert_eq!(parse_sync_ttl("soon"), 3600);
    assert_eq!(parse_sync_ttl("1.5"), 3600);
}

/// End-to-end through Config::from_env with the variable actually set, so a
/// regression that stops routing SYNC_TTL_SECS through parse_sync_ttl fails
/// here. This is the only test that mutates the variable, so it cannot race.
#[test]
fn sync_ttl_from_env_is_clamped() {
    std::env::set_var("SYNC_TTL_SECS", "0");
    assert_eq!(Config::from_env().sync_ttl_secs, 3600);
    std::env::set_var("SYNC_TTL_SECS", "7200");
    assert_eq!(Config::from_env().sync_ttl_secs, 7200);
    std::env::remove_var("SYNC_TTL_SECS");
    assert_eq!(Config::from_env().sync_ttl_secs, 3600);
}

// ---------------------------------------------------------------------------
// ratelimit: token bucket
// ---------------------------------------------------------------------------

#[test]
fn ratelimit_burst_allowed_then_denied() {
    // rate 0 => no refill, so the outcome is exact regardless of timing.
    let rl = RateLimiter::new(0.0, 3.0);
    assert!(rl.check("v1"));
    assert!(rl.check("v1"));
    assert!(rl.check("v1"));
    assert!(!rl.check("v1"), "4th call must exceed the burst of 3");
    assert!(!rl.check("v1"), "stays denied without refill");
}

#[test]
fn ratelimit_keys_are_independent() {
    let rl = RateLimiter::new(0.0, 1.0);
    assert!(rl.check("a"));
    assert!(!rl.check("a"));
    assert!(rl.check("b"), "a different key gets its own bucket");
}

#[test]
fn ratelimit_refills_over_time() {
    // 6000/min = 100 tokens/sec. Drain the burst, then sleeping >= 50ms
    // guarantees at least 5 tokens accrue (sleep never wakes early).
    let rl = RateLimiter::new(6000.0, 2.0);
    assert!(rl.check("v"));
    assert!(rl.check("v"));
    assert!(!rl.check("v"), "burst drained");
    std::thread::sleep(Duration::from_millis(50));
    assert!(rl.check("v"), "refill must restore at least one token");
}

#[test]
fn ratelimit_refill_capped_at_burst() {
    // After 120ms at 100 tokens/sec ~12 tokens would accrue uncapped; the
    // bucket must hold at most `burst` (2). Allow one extra success in case
    // the scheduler pauses long enough mid-loop to refill a whole token.
    let rl = RateLimiter::new(6000.0, 2.0);
    assert!(rl.check("v"));
    assert!(rl.check("v"));
    std::thread::sleep(Duration::from_millis(120));
    let allowed = (0..10).filter(|_| rl.check("v")).count();
    assert!((2..=3).contains(&allowed), "expected ~burst successes, got {allowed}");
}

// ---------------------------------------------------------------------------
// visitor: cookie signing
// ---------------------------------------------------------------------------

/// The hand-rolled HMAC-SHA256 must match known test vectors.
#[test]
fn hmac_matches_known_vectors() {
    // Classic "quick brown fox" vector (key shorter than the block size).
    assert_eq!(
        sign(b"key", "The quick brown fox jumps over the lazy dog"),
        "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8"
    );
    // RFC 4231 test case 6: 131-byte key exercises the hash-the-key branch.
    assert_eq!(
        sign(&[0xaa; 131], "Test Using Larger Than Block-Size Key - Hash Key First"),
        "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"
    );
}

#[test]
fn cookie_sign_verify_roundtrip() {
    let id = uuid::Uuid::new_v4().to_string();
    let value = make_cookie_value(SECRET, &id);
    assert!(value.starts_with(&format!("{id}.")));
    // A returning visitor presents this value and gets the same id back.
    assert_eq!(verify(SECRET, &value), Some(id));
}

#[test]
fn cookie_tampered_signature_rejected() {
    let value = make_cookie_value(SECRET, "visitor-1");
    // Flip the last hex digit of the signature.
    let last = value.chars().last().unwrap();
    let flipped = if last == '0' { '1' } else { '0' };
    let mut tampered = value[..value.len() - 1].to_string();
    tampered.push(flipped);
    assert_eq!(verify(SECRET, &tampered), None);
}

#[test]
fn cookie_swapped_id_rejected() {
    // Reusing a valid signature with a different id must fail.
    let value = make_cookie_value(SECRET, "visitor-1");
    let (_, sig) = value.split_once('.').unwrap();
    assert_eq!(verify(SECRET, &format!("visitor-2.{sig}")), None);
}

#[test]
fn cookie_wrong_secret_rejected() {
    let value = make_cookie_value(SECRET, "visitor-1");
    assert_eq!(verify(b"a-completely-different-secret!!!", &value), None);
}

#[test]
fn cookie_malformed_value_rejected() {
    // No '.' separator at all (a fresh visitor is minted instead).
    assert_eq!(verify(SECRET, "no-separator-here"), None);
    assert_eq!(verify(SECRET, ""), None);
}

#[test]
fn unlock_cookie_shape_and_domain_separation() {
    let cookie = unlock_set_cookie(SECRET, "tenant-a");
    assert!(cookie.starts_with("u_tenant-a="));
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("SameSite=Lax"));

    let sig_of = |c: &str| c.split_once('=').unwrap().1.split(';').next().unwrap().to_string();
    let sig = sig_of(&cookie);
    assert_eq!(sig.len(), 64);
    assert!(sig.chars().all(|c| c.is_ascii_hexdigit()));
    // Deterministic per (secret, tenant); distinct across tenants.
    assert_eq!(sig, sig_of(&unlock_set_cookie(SECRET, "tenant-a")));
    assert_ne!(sig, sig_of(&unlock_set_cookie(SECRET, "tenant-b")));
    // Domain separation: an unlock sig must not equal a plain visitor sig of
    // the tenant id, or a signed vid cookie could double as an unlock.
    assert_ne!(sig, sign(SECRET, "tenant-a"));
}

// ---------------------------------------------------------------------------
// download: safe_name
// ---------------------------------------------------------------------------

#[test]
fn safe_name_strips_path_traversal() {
    assert_eq!(safe_name("../../etc/passwd"), ".._.._etc_passwd");
    assert_eq!(safe_name("..\\..\\evil.jpg"), ".._.._evil.jpg");
}

#[test]
fn safe_name_strips_header_injection_chars() {
    // Quotes and CR/LF could break out of the Content-Disposition header.
    assert_eq!(safe_name("a\"b\r\n.jpg"), "a_b__.jpg");
}

#[test]
fn safe_name_empty_or_blank_becomes_file() {
    assert_eq!(safe_name(""), "file");
    assert_eq!(safe_name("   "), "file");
}

#[test]
fn safe_name_keeps_ordinary_names() {
    assert_eq!(safe_name("IMG_1234 (edited).jpg"), "IMG_1234 (edited).jpg");
}

// ---------------------------------------------------------------------------
// visitor: sqlite-backed helpers
// ---------------------------------------------------------------------------

/// In-memory pool with migrations applied. max_connections(1): each sqlite
/// :memory: connection is its own database, so the pool must not open more.
async fn test_db() -> sqlx::SqlitePool {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("open in-memory sqlite");
    sqlx::migrate!("./migrations").run(&pool).await.expect("run migrations");
    pool
}

#[tokio::test]
async fn visitor_display_name_and_ban_flow() {
    let db = test_db().await;

    // Unknown visitor: empty name, not banned.
    assert_eq!(crate::visitor::display_name(&db, "ghost").await, "");
    assert!(!crate::visitor::is_banned(&db, "ghost").await);

    sqlx::query("INSERT INTO visitor (id, display_name, first_seen) VALUES (?, ?, ?)")
        .bind("v-1")
        .bind("Oma")
        .bind(crate::state::now())
        .execute(&db)
        .await
        .unwrap();

    assert_eq!(crate::visitor::display_name(&db, "v-1").await, "Oma");
    assert!(!crate::visitor::is_banned(&db, "v-1").await);

    sqlx::query("UPDATE visitor SET banned = 1 WHERE id = ?")
        .bind("v-1")
        .execute(&db)
        .await
        .unwrap();
    assert!(crate::visitor::is_banned(&db, "v-1").await);
}

// ---------------------------------------------------------------------------
// albums: public index listing
// ---------------------------------------------------------------------------

async fn insert_tenant(db: &sqlx::SqlitePool, id: &str, key: &str, needs_password: i64) {
    // `listed` is deliberately omitted so its schema default applies.
    sqlx::query(
        "INSERT INTO tenant (id, share_key, immich_album, title, needs_password, created_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(key)
    .bind(format!("album-{id}"))
    .bind(format!("Title {id}"))
    .bind(needs_password)
    .bind(crate::state::now())
    .execute(db)
    .await
    .unwrap();
}

async fn insert_asset(db: &sqlx::SqlitePool, tenant: &str, id: &str, kind: &str, taken_at: i64) {
    sqlx::query("INSERT INTO asset (tenant_id, asset_id, kind, taken_at) VALUES (?, ?, ?, ?)")
        .bind(tenant)
        .bind(id)
        .bind(kind)
        .bind(taken_at)
        .execute(db)
        .await
        .unwrap();
}

#[tokio::test]
async fn album_index_lists_tenants_with_password_flag() {
    let db = test_db().await;

    // One public tenant, one password-protected: BOTH are listed, the
    // protected one flagged so the index can show a lock.
    insert_tenant(&db, "t-pub", "key-pub", 0).await;
    insert_tenant(&db, "t-pw", "key-pw", 1).await;
    insert_asset(&db, "t-pub", "a-old", "IMAGE", 100).await;
    insert_asset(&db, "t-pub", "a-new", "IMAGE", 200).await;
    insert_asset(&db, "t-pub", "a-vid", "VIDEO", 150).await;
    insert_asset(&db, "t-pw", "b-1", "IMAGE", 100).await;
    insert_asset(&db, "t-pw", "b-2", "VIDEO", 200).await;

    // The newly inserted rows (which omitted the column) default to listed=1.
    let listed: (i64,) = sqlx::query_as("SELECT listed FROM tenant WHERE id = 't-pub'")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(listed.0, 1);

    let rows = crate::routes::albums::listed_albums(&db).await.unwrap();
    assert_eq!(rows.len(), 2, "both tenants are listed");
    let pubrow = rows.iter().find(|a| a.key == "key-pub").unwrap();
    assert_eq!(pubrow.title.as_deref(), Some("Title t-pub"));
    assert_eq!(pubrow.photos, 2);
    assert_eq!(pubrow.videos, 1);
    assert!(!pubrow.needs_password);
    let pwrow = rows.iter().find(|a| a.key == "key-pw").unwrap();
    assert!(pwrow.needs_password, "protected tenant carries the lock flag");

    // Unlisting hides a tenant from the index entirely.
    sqlx::query("UPDATE tenant SET listed = 0 WHERE id IN ('t-pub', 't-pw')")
        .execute(&db)
        .await
        .unwrap();
    assert!(crate::routes::albums::listed_albums(&db).await.unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// tenant: dead-share detection helpers
// ---------------------------------------------------------------------------

#[tokio::test]
async fn mark_tenant_dead_delists_and_keeps_first_timestamp() {
    let db = test_db().await;
    insert_tenant(&db, "t-1", "key-1", 0).await;

    // First detection: delisted, dead_since stamped, reported as new.
    assert!(crate::tenant::mark_tenant_dead(&db, "t-1", 1000).await.unwrap());
    let (listed, dead): (i64, Option<i64>) =
        sqlx::query_as("SELECT listed, dead_since FROM tenant WHERE id = 't-1'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(listed, 0);
    assert_eq!(dead, Some(1000));

    // Repeat detection keeps the ORIGINAL timestamp and is not "new".
    assert!(!crate::tenant::mark_tenant_dead(&db, "t-1", 2000).await.unwrap());
    let (dead,): (Option<i64>,) =
        sqlx::query_as("SELECT dead_since FROM tenant WHERE id = 't-1'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(dead, Some(1000), "first detection time survives repeats");
}

#[tokio::test]
async fn mark_tenant_needs_password_gates_and_drops_stale_token() {
    let db = test_db().await;
    insert_tenant(&db, "t-1", "key-1", 0).await;
    sqlx::query("UPDATE tenant SET share_token = 'stale-token' WHERE id = 't-1'")
        .execute(&db)
        .await
        .unwrap();

    crate::tenant::mark_tenant_needs_password(&db, "t-1").await.unwrap();
    let (needs, token): (i64, String) =
        sqlx::query_as("SELECT needs_password, share_token FROM tenant WHERE id = 't-1'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(needs, 1);
    assert_eq!(token, "", "rejected token is cleared so the gate re-prompts");
}

#[tokio::test]
async fn clear_tenant_dead_nulls_marker_but_not_listed() {
    let db = test_db().await;
    insert_tenant(&db, "t-1", "key-1", 0).await;
    crate::tenant::mark_tenant_dead(&db, "t-1", 1000).await.unwrap();

    crate::tenant::clear_tenant_dead(&db, "t-1").await.unwrap();
    let (listed, dead): (i64, Option<i64>) =
        sqlx::query_as("SELECT listed, dead_since FROM tenant WHERE id = 't-1'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(dead, None);
    assert_eq!(listed, 0, "relisting stays a human decision");
}

#[tokio::test]
async fn dead_tenant_disappears_from_public_index() {
    let db = test_db().await;
    insert_tenant(&db, "t-1", "key-1", 0).await;
    insert_tenant(&db, "t-2", "key-2", 0).await;
    assert_eq!(crate::routes::albums::listed_albums(&db).await.unwrap().len(), 2);

    crate::tenant::mark_tenant_dead(&db, "t-1", 1000).await.unwrap();
    let rows = crate::routes::albums::listed_albums(&db).await.unwrap();
    assert_eq!(rows.len(), 1, "dead tenant is delisted");
    assert_eq!(rows[0].key, "key-2");
}

#[tokio::test]
async fn purge_tenant_removes_all_tenant_data_and_nothing_else() {
    let db = test_db().await;

    insert_tenant(&db, "t-dead", "key-dead", 0).await;
    insert_tenant(&db, "t-live", "key-live", 0).await;
    insert_asset(&db, "t-dead", "a-1", "IMAGE", 100).await;
    insert_asset(&db, "t-dead", "a-2", "VIDEO", 200).await;
    insert_asset(&db, "t-live", "b-1", "IMAGE", 100).await;
    sqlx::query("INSERT INTO mark (tenant_id, asset_id, visitor_id, created_at) VALUES ('t-dead', 'a-1', 'v', 1)")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO mark (tenant_id, asset_id, visitor_id, created_at) VALUES ('t-live', 'b-1', 'v', 1)")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO note (tenant_id, asset_id, visitor_id, body, created_at) VALUES ('t-dead', 'a-1', 'v', 'hi', 1)")
        .execute(&db)
        .await
        .unwrap();

    let (assets, marks, notes, tenants) =
        crate::routes::admin::purge_tenant(&db, "t-dead").await.unwrap();
    assert_eq!((assets, marks, notes, tenants), (2, 1, 1, 1));

    // The other tenant's data is untouched.
    let live: (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM asset WHERE tenant_id = 't-live'), \
                (SELECT COUNT(*) FROM mark WHERE tenant_id = 't-live'), \
                (SELECT COUNT(*) FROM tenant WHERE id = 't-live')",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(live, (1, 1, 1));

    // Purging an unknown id deletes nothing (handler turns this into 404).
    let (_, _, _, tenants) = crate::routes::admin::purge_tenant(&db, "nope").await.unwrap();
    assert_eq!(tenants, 0);
}
