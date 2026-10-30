//! Read-only, bounded access to the same fixed manifest used by Tauri Updater.
//! This command never downloads or installs an executable.
use reqwest::blocking::Client;
use reqwest::header::HeaderMap;
use serde::Serialize;
use std::io::Read;
use std::time::Duration;

const ENDPOINT: &str =
    "https://github.com/GKNEETIEMAY/ProxyEnv/releases/latest/download/latest.json";
const MAX_BYTES: u64 = 128 * 1024;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MetadataError {
    category: &'static str,
    retry_at: Option<i64>,
}
fn error(category: &'static str) -> MetadataError {
    MetadataError {
        category,
        retry_at: None,
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct PublishedMetadata {
    version: String,
    body: String,
    date: String,
}

fn retry_at(headers: &HeaderMap, now: i64) -> i64 {
    let retry = headers
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.parse::<i64>()
                .ok()
                .filter(|seconds| *seconds >= 0)
                .map(|seconds| now.saturating_add(seconds))
                .or_else(|| {
                    chrono::DateTime::parse_from_rfc2822(v)
                        .ok()
                        .map(|date| date.timestamp())
                })
        });
    let reset = headers
        .get("x-ratelimit-reset")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<i64>().ok());
    retry
        .into_iter()
        .chain(reset)
        .max()
        .unwrap_or(now + 60)
        .clamp(now + 60, now + 7 * 24 * 3600)
        * 1000
}

fn parse_manifest(bytes: &[u8]) -> Result<PublishedMetadata, MetadataError> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| error("invalidResponse"))?;
    let version = value["version"]
        .as_str()
        .ok_or_else(|| error("invalidResponse"))?;
    let version = version.strip_prefix('v').unwrap_or(version);
    let parts: Vec<_> = version.split('.').collect();
    if parts.len() != 3
        || parts.iter().any(|part| {
            part.is_empty()
                || (part.len() > 1 && part.starts_with('0'))
                || !part.bytes().all(|b| b.is_ascii_digit())
                || part
                    .parse::<u64>()
                    .map_or(true, |n| n > 9_007_199_254_740_991)
        })
    {
        return Err(error("invalidResponse"));
    }
    let body = value["notes"]
        .as_str()
        .ok_or_else(|| error("invalidResponse"))?;
    let date = value["pub_date"]
        .as_str()
        .ok_or_else(|| error("invalidResponse"))?;
    let platform = &value["platforms"]["windows-x86_64"];
    let expected = format!("https://github.com/GKNEETIEMAY/ProxyEnv/releases/download/v{version}/RouteSpan-{version}-windows-x64-setup.exe");
    let signature = platform["signature"].as_str().unwrap_or_default();
    if body.len() > 80_000
        || chrono::DateTime::parse_from_rfc3339(date).is_err()
        || platform["url"].as_str() != Some(expected.as_str())
        || signature.trim().is_empty()
        || signature.len() > 4096
    {
        return Err(error("invalidResponse"));
    }
    Ok(PublishedMetadata {
        version: version.into(),
        body: body.into(),
        date: date.into(),
    })
}

fn transport_error(cause: reqwest::Error) -> MetadataError {
    if cause.is_timeout() {
        return error("timeout");
    }
    // Classify privately; never return URLs, local paths or raw transport errors.
    let text = format!("{cause:?}").to_lowercase();
    if text.contains("certificate") || text.contains("tls") {
        error("tls")
    } else if text.contains("proxy") || text.contains("407") {
        error("proxy")
    } else {
        error("network")
    }
}

fn response_failure(status: u16, headers: &HeaderMap, now: i64) -> Option<MetadataError> {
    if status == 429
        || (status == 403
            && (headers
                .get("x-ratelimit-remaining")
                .is_some_and(|v| v == "0")
                || headers.contains_key("retry-after")))
    {
        Some(MetadataError {
            category: "rateLimit",
            retry_at: Some(retry_at(headers, now)),
        })
    } else if status == 407 {
        Some(error("proxy"))
    } else if !(200..300).contains(&status) {
        Some(error("service"))
    } else {
        None
    }
}

fn fetch_metadata() -> Result<Option<PublishedMetadata>, MetadataError> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .https_only(true)
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(transport_error)?;
    let response = client
        .get(ENDPOINT)
        .header("Accept", "application/json")
        .send()
        .map_err(transport_error)?;
    let status = response.status();
    if status.as_u16() == 404 {
        return Ok(None);
    }
    if let Some(failure) = response_failure(
        status.as_u16(),
        response.headers(),
        chrono::Utc::now().timestamp(),
    ) {
        return Err(failure);
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_BYTES)
    {
        return Err(error("invalidResponse"));
    }
    let mut bytes = Vec::new();
    response
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| error("network"))?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(error("invalidResponse"));
    }
    parse_manifest(&bytes).map(Some)
}

#[tauri::command]
pub(crate) async fn fetch_update_metadata() -> Result<Option<PublishedMetadata>, MetadataError> {
    tauri::async_runtime::spawn_blocking(fetch_metadata)
        .await
        .map_err(|_| error("service"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn status_requires_evidence_for_403_and_recognizes_429_and_proxy_auth() {
        let mut headers = HeaderMap::new();
        assert!(response_failure(200, &headers, 100).is_none());
        for (status, category) in [
            (403, "service"),
            (429, "rateLimit"),
            (407, "proxy"),
            (500, "service"),
        ] {
            assert_eq!(
                response_failure(status, &headers, 100).unwrap().category,
                category
            );
        }
        headers.insert("x-ratelimit-remaining", "12".parse().unwrap());
        assert_eq!(
            response_failure(403, &headers, 100).unwrap().category,
            "service"
        );
        headers.insert("x-ratelimit-remaining", "0".parse().unwrap());
        assert_eq!(
            response_failure(403, &headers, 100).unwrap().retry_at,
            Some(160_000)
        );
        headers.remove("x-ratelimit-remaining");
        headers.insert("retry-after", "120".parse().unwrap());
        assert_eq!(
            response_failure(403, &headers, 100).unwrap().retry_at,
            Some(220_000)
        );
    }
    fn manifest() -> serde_json::Value {
        serde_json::json!({"version":"0.2.2","notes":"notes","pub_date":"2026-10-30T00:00:00Z",
            "platforms":{"windows-x86_64":{"signature":"fixture-not-a-real-signature",
                "url":"https://github.com/GKNEETIEMAY/ProxyEnv/releases/download/v0.2.2/RouteSpan-0.2.2-windows-x64-setup.exe"}}})
    }
    #[test]
    fn manifest_rejects_untrusted_targets_and_invalid_versions() {
        assert_eq!(
            parse_manifest(&serde_json::to_vec(&manifest()).unwrap())
                .unwrap()
                .version,
            "0.2.2"
        );
        for version in ["0.2.2-rc.1", "0.02.2", "0.2", "0.2.9007199254740992"] {
            let mut value = manifest();
            value["version"] = version.into();
            assert!(parse_manifest(&serde_json::to_vec(&value).unwrap()).is_err());
        }
        for (field, invalid) in [("url", "https://example.com/app.exe"), ("url", "https://github.com/GKNEETIEMAY/ProxyEnv/releases/download/v0.2.1/RouteSpan-0.2.1-windows-x64-setup.exe"), ("signature", "")] {
            let mut value = manifest(); value["platforms"]["windows-x86_64"][field] = invalid.into();
            assert!(parse_manifest(&serde_json::to_vec(&value).unwrap()).is_err());
        }
        let mut value = manifest();
        value["notes"] = "x".repeat(80_001).into();
        assert!(parse_manifest(&serde_json::to_vec(&value).unwrap()).is_err());
    }
    #[test]
    fn cooldown_obeys_both_headers_and_has_bounded_fallback() {
        let now = 1_700_000_000;
        let mut headers = HeaderMap::new();
        assert_eq!(retry_at(&headers, now), (now + 60) * 1000);
        headers.insert("retry-after", "120".parse().unwrap());
        headers.insert(
            "x-ratelimit-reset",
            (now + 300).to_string().parse().unwrap(),
        );
        assert_eq!(retry_at(&headers, now), (now + 300) * 1000);
        headers.remove("x-ratelimit-reset");
        headers.insert(
            "retry-after",
            chrono::DateTime::from_timestamp(now + 600, 0)
                .unwrap()
                .to_rfc2822()
                .parse()
                .unwrap(),
        );
        assert_eq!(retry_at(&headers, now), (now + 600) * 1000);
        headers.insert("retry-after", "9223372036854775807".parse().unwrap());
        assert_eq!(retry_at(&headers, now), (now + 7 * 24 * 3600) * 1000);
    }
}
