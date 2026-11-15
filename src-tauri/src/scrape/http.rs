//! One HTTP client for every scraper: polite per-source rate limiting,
//! retries with backoff on 429/5xx/network errors, and a disk cache for
//! downloaded images so nothing is fetched twice.

use crate::error::{AppError, AppResult};
use reqwest::blocking::{Client, RequestBuilder, Response};
use reqwest::StatusCode;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const USER_AGENT: &str = concat!(
    "FlashLauncher/",
    env!("CARGO_PKG_VERSION"),
    " (personal Flash game launcher)"
);
const MAX_IMAGE_BYTES: usize = 25 * 1024 * 1024;

pub struct Http {
    client: Client,
    last: Mutex<HashMap<&'static str, Instant>>,
    cache_dir: PathBuf,
}

/// Minimum gap between requests to the same source.
fn min_gap(source: &str) -> Duration {
    match source {
        "igdb" => Duration::from_millis(260), // documented limit: 4 req/s
        "archive" => Duration::from_millis(600),
        "flashpoint" => Duration::from_millis(350), // community-run, be gentle
        _ => Duration::from_millis(200),
    }
}

impl Http {
    pub fn new(cache_dir: PathBuf) -> AppResult<Self> {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(25))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| AppError(format!("couldn't set up the HTTP client: {e}")))?;
        std::fs::create_dir_all(&cache_dir)?;
        Ok(Self {
            client,
            last: Mutex::new(HashMap::new()),
            cache_dir,
        })
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    fn wait_turn(&self, source: &'static str) {
        let gap = min_gap(source);
        let sleep_for = {
            let mut last = self.last.lock().unwrap_or_else(|e| e.into_inner());
            let now = Instant::now();
            let next = last.get(source).map(|t| *t + gap).unwrap_or(now);
            let start = next.max(now);
            last.insert(source, start);
            start - now
        };
        if !sleep_for.is_zero() {
            std::thread::sleep(sleep_for);
        }
    }

    /// Sends a request built by `make`, retrying transient failures.
    pub fn send(
        &self,
        source: &'static str,
        make: impl Fn(&Client) -> RequestBuilder,
    ) -> AppResult<Response> {
        let mut delay = Duration::from_millis(600);
        let mut last_err = String::new();
        for attempt in 0..3 {
            self.wait_turn(source);
            match make(&self.client).send() {
                Ok(r) if r.status() == StatusCode::TOO_MANY_REQUESTS || r.status().is_server_error() => {
                    last_err = format!("{} answered {}", source, r.status());
                    let retry_after = r
                        .headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.parse::<u64>().ok())
                        .map(Duration::from_secs);
                    if attempt < 2 {
                        std::thread::sleep(retry_after.unwrap_or(delay).min(Duration::from_secs(10)));
                    }
                }
                Ok(r) if r.status() == StatusCode::UNAUTHORIZED || r.status() == StatusCode::FORBIDDEN => {
                    return Err(AppError(format!(
                        "{source} rejected the request ({}). Check the API key in settings.",
                        r.status()
                    )));
                }
                Ok(r) if !r.status().is_success() => {
                    return Err(AppError(format!("{source} answered {}", r.status())));
                }
                Ok(r) => return Ok(r),
                Err(e) => {
                    last_err = if e.is_timeout() {
                        format!("{source} timed out")
                    } else if e.is_connect() {
                        format!("couldn't reach {source}. Are you online?")
                    } else {
                        format!("{source}: {e}")
                    };
                    if attempt < 2 {
                        std::thread::sleep(delay);
                    }
                }
            }
            delay *= 2;
        }
        Err(AppError(last_err))
    }

    pub fn json(
        &self,
        source: &'static str,
        make: impl Fn(&Client) -> RequestBuilder,
    ) -> AppResult<serde_json::Value> {
        let r = self.send(source, make)?;
        r.json()
            .map_err(|e| AppError(format!("{source} sent something that isn't valid JSON: {e}")))
    }

    /// Downloads an image, using the disk cache when we've seen this URL.
    pub fn image(&self, source: &'static str, url: &str) -> AppResult<Vec<u8>> {
        let key: String = Sha256::digest(url.as_bytes())
            .iter()
            .take(16)
            .map(|b| format!("{b:02x}"))
            .collect();
        let cached = self.cache_dir.join(&key);
        if let Ok(bytes) = std::fs::read(&cached) {
            return Ok(bytes);
        }
        let r = self.send(source, |c| c.get(url))?;
        let bytes = r
            .bytes()
            .map_err(|e| AppError(format!("download from {source} failed: {e}")))?;
        if bytes.len() > MAX_IMAGE_BYTES {
            return Err(AppError("image is too large".into()));
        }
        let _ = std::fs::write(&cached, &bytes);
        Ok(bytes.to_vec())
    }
}
