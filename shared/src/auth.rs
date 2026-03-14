use std::time::{SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

/// Maximum allowed clock drift (seconds) between agent and collector.
const MAX_DRIFT_SECS: u64 = 300;

/// Errors that [`verify_auth_header`] can return.
#[derive(Debug, PartialEq)]
pub enum AuthError {
    /// No `Authorization` header was present (or the value was empty).
    Missing,
    /// The header value did not match the expected `HMAC a:ts:sha:sig` shape.
    Malformed,
    /// The HMAC signature did not match the recomputed value.
    InvalidSignature,
    /// The timestamp in the header is more than 300 s from the current time.
    TimestampExpired,
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::Missing => write!(f, "Authorization header is missing"),
            AuthError::Malformed => write!(f, "Authorization header is malformed"),
            AuthError::InvalidSignature => write!(f, "HMAC signature is invalid"),
            AuthError::TimestampExpired => {
                write!(f, "Request timestamp is outside the 300 s window")
            }
        }
    }
}

/// Returns the current Unix timestamp in whole seconds.
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Compute the SHA-256 digest of `data` and return it as a lowercase hex string.
fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

/// Compute the Authorization header value for a payload.
///
/// The format is:
/// ```text
/// HMAC <agent_id>:<unix_ts_secs>:<body_sha256_hex>:<hmac_hex>
/// ```
/// where the HMAC is computed over the string `"<agent_id>:<ts>:<body_sha256_hex>"`.
///
/// Returns `None` when `secret` is empty, meaning auth is disabled and the
/// caller should omit the header entirely (backward-compatible dev mode).
pub fn make_auth_header(secret: &str, agent_id: &str, body_bytes: &[u8]) -> Option<String> {
    if secret.is_empty() {
        return None;
    }

    let ts = now_secs();
    let body_sha256 = sha256_hex(body_bytes);
    let message = format!("{agent_id}:{ts}:{body_sha256}");

    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts keys of any length");
    mac.update(message.as_bytes());
    let sig = hex::encode(mac.finalize().into_bytes());

    Some(format!("HMAC {agent_id}:{ts}:{body_sha256}:{sig}"))
}

/// Verify an `Authorization` header value against a payload.
///
/// Returns `Ok(())` on success or `Err(`[`AuthError`]`)` on any failure.
///
/// When `secret` is empty auth is **disabled** and the function always returns
/// `Ok(())` regardless of the header value, preserving backward compatibility.
pub fn verify_auth_header(
    secret: &str,
    header_value: &str,
    body_bytes: &[u8],
) -> Result<(), AuthError> {
    // Auth disabled — accept everything.
    if secret.is_empty() {
        return Ok(());
    }

    // Reject missing / blank header.
    if header_value.is_empty() {
        return Err(AuthError::Missing);
    }

    // Strip the "HMAC " prefix.
    let rest = header_value
        .strip_prefix("HMAC ")
        .ok_or(AuthError::Malformed)?;

    // Split into exactly 4 parts: agent_id, ts_str, body_sha256, provided_sig.
    // We must be careful: agent_id might theoretically contain colons, but by
    // design it does not.  We split into at most 4 parts left-to-right so that
    // the SHA-256 hex (which never contains colons) and sig parse cleanly.
    let parts: Vec<&str> = rest.splitn(4, ':').collect();
    if parts.len() != 4 {
        return Err(AuthError::Malformed);
    }
    let (agent_id, ts_str, header_sha256, provided_sig) = (parts[0], parts[1], parts[2], parts[3]);

    if agent_id.is_empty()
        || ts_str.is_empty()
        || header_sha256.is_empty()
        || provided_sig.is_empty()
    {
        return Err(AuthError::Malformed);
    }

    // --- Replay-protection: check timestamp drift ---
    let ts: u64 = ts_str.parse().map_err(|_| AuthError::Malformed)?;
    let now = now_secs();
    let drift = now.abs_diff(ts);
    if drift > MAX_DRIFT_SECS {
        return Err(AuthError::TimestampExpired);
    }

    // --- Body integrity: recompute SHA-256 and compare ---
    let expected_sha256 = sha256_hex(body_bytes);
    if expected_sha256 != header_sha256 {
        return Err(AuthError::InvalidSignature);
    }

    // --- HMAC verification using constant-time comparison ---
    let message = format!("{agent_id}:{ts_str}:{header_sha256}");
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts keys of any length");
    mac.update(message.as_bytes());

    // `verify_slice` does a constant-time comparison internally.
    let provided_bytes = hex::decode(provided_sig).map_err(|_| AuthError::Malformed)?;
    mac.verify_slice(&provided_bytes)
        .map_err(|_| AuthError::InvalidSignature)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "test-secret-key";
    const AGENT: &str = "agent-01";
    const BODY: &[u8] = b"{\"agent_id\":\"agent-01\"}";

    // ── make_auth_header ──────────────────────────────────────────────────────

    #[test]
    fn make_returns_none_for_empty_secret() {
        assert!(make_auth_header("", AGENT, BODY).is_none());
    }

    #[test]
    fn make_returns_some_with_correct_prefix() {
        let hdr = make_auth_header(SECRET, AGENT, BODY).unwrap();
        assert!(hdr.starts_with("HMAC agent-01:"), "got: {hdr}");
    }

    #[test]
    fn make_header_has_four_colon_separated_parts_after_prefix() {
        let hdr = make_auth_header(SECRET, AGENT, BODY).unwrap();
        let rest = hdr.strip_prefix("HMAC ").unwrap();
        // splitn(4, ':') should yield exactly 4 non-empty parts
        let parts: Vec<&str> = rest.splitn(4, ':').collect();
        assert_eq!(parts.len(), 4);
        for p in &parts {
            assert!(!p.is_empty());
        }
    }

    // ── verify_auth_header ───────────────────────────────────────────────────

    #[test]
    fn verify_accepts_empty_secret_regardless_of_header() {
        assert!(verify_auth_header("", "garbage", BODY).is_ok());
        assert!(verify_auth_header("", "", BODY).is_ok());
    }

    #[test]
    fn roundtrip_valid_header_is_accepted() {
        let hdr = make_auth_header(SECRET, AGENT, BODY).unwrap();
        assert!(verify_auth_header(SECRET, &hdr, BODY).is_ok());
    }

    #[test]
    fn verify_rejects_missing_header() {
        assert_eq!(
            verify_auth_header(SECRET, "", BODY),
            Err(AuthError::Missing)
        );
    }

    #[test]
    fn verify_rejects_malformed_header_no_prefix() {
        assert_eq!(
            verify_auth_header(SECRET, "Bearer some-token", BODY),
            Err(AuthError::Malformed)
        );
    }

    #[test]
    fn verify_rejects_malformed_header_too_few_parts() {
        // Only 3 colon-separated parts instead of 4.
        assert_eq!(
            verify_auth_header(SECRET, "HMAC agent:12345:deadbeef", BODY),
            Err(AuthError::Malformed)
        );
    }

    #[test]
    fn verify_rejects_wrong_signature() {
        let hdr = make_auth_header(SECRET, AGENT, BODY).unwrap();
        // Flip the last character of the header to corrupt the HMAC.
        let corrupted = {
            let mut s = hdr.clone();
            let last = s.pop().unwrap();
            let replacement = if last == 'a' { 'b' } else { 'a' };
            s.push(replacement);
            s
        };
        assert_eq!(
            verify_auth_header(SECRET, &corrupted, BODY),
            Err(AuthError::InvalidSignature)
        );
    }

    #[test]
    fn verify_rejects_wrong_secret() {
        let hdr = make_auth_header(SECRET, AGENT, BODY).unwrap();
        assert_eq!(
            verify_auth_header("wrong-secret", &hdr, BODY),
            Err(AuthError::InvalidSignature)
        );
    }

    #[test]
    fn verify_rejects_tampered_body() {
        let hdr = make_auth_header(SECRET, AGENT, BODY).unwrap();
        // Body is different from what was signed.
        assert_eq!(
            verify_auth_header(SECRET, &hdr, b"tampered body"),
            Err(AuthError::InvalidSignature)
        );
    }

    #[test]
    fn verify_rejects_expired_timestamp() {
        // Craft a header with a timestamp 400 seconds in the past.
        let old_ts = now_secs().saturating_sub(400);
        let body_sha256 = sha256_hex(BODY);
        let message = format!("{AGENT}:{old_ts}:{body_sha256}");
        let mut mac = HmacSha256::new_from_slice(SECRET.as_bytes()).unwrap();
        mac.update(message.as_bytes());
        let sig = hex::encode(mac.finalize().into_bytes());
        let hdr = format!("HMAC {AGENT}:{old_ts}:{body_sha256}:{sig}");

        assert_eq!(
            verify_auth_header(SECRET, &hdr, BODY),
            Err(AuthError::TimestampExpired)
        );
    }

    #[test]
    fn verify_accepts_timestamp_within_window() {
        // Timestamp 10 seconds in the past — well within the 300 s window.
        let recent_ts = now_secs().saturating_sub(10);
        let body_sha256 = sha256_hex(BODY);
        let message = format!("{AGENT}:{recent_ts}:{body_sha256}");
        let mut mac = HmacSha256::new_from_slice(SECRET.as_bytes()).unwrap();
        mac.update(message.as_bytes());
        let sig = hex::encode(mac.finalize().into_bytes());
        let hdr = format!("HMAC {AGENT}:{recent_ts}:{body_sha256}:{sig}");

        assert!(verify_auth_header(SECRET, &hdr, BODY).is_ok());
    }
}
