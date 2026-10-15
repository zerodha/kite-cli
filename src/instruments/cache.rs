use super::{IST_OFFSET_SECS, Instrument, InstrumentStore};
use crate::app::http::{DOWNLOAD_TIMEOUT, client_builder};
use kiteconnect::DEFAULT_BASE_URL as BASE_URL;
use reqwest::header::{ACCEPT, HeaderMap, HeaderValue};
use serde::Serialize;
use std::io::{BufRead, BufReader, Cursor};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

const INSTRUMENTS_JSON_PATH: &str = "/instruments.json";
const PUBLISH_HOUR_IST: u64 = 7;
static DOWNLOAD_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize)]
pub struct CacheInfo {
    pub path: PathBuf,
    pub exists: bool,
    pub stale: bool,
    /// The first publication cutoff after the cache was written.
    /// `None` when no cache timestamp is available.
    pub fresh_until_unix_seconds: Option<u64>,
    pub size_bytes: Option<u64>,
    pub modified_unix_seconds: Option<u64>,
    pub record_count: Option<usize>,
}

/// # Errors
/// Returns an error if no usable cache can be loaded and downloading or parsing fails.
pub async fn load(app_uuid: &str) -> Result<InstrumentStore, String> {
    if let Some(path) = cache_path()
        && cache_is_fresh(&path)
        && let Ok(store) = load_store_from_path(&path).await
        && !store.is_empty()
    {
        return Ok(store);
    }

    refresh(app_uuid).await
}

/// Download and parse the master, publishing the cache only after parsing succeeds.
///
/// # Errors
/// Returns an error if downloading or parsing the instrument master fails.
pub async fn refresh(app_uuid: &str) -> Result<InstrumentStore, String> {
    let client = download_client(app_uuid)?;
    refresh_from(
        &format!("{BASE_URL}{INSTRUMENTS_JSON_PATH}"),
        cache_path().as_deref(),
        &client,
    )
    .await
}

async fn refresh_from(
    url: &str,
    path: Option<&Path>,
    client: &reqwest::Client,
) -> Result<InstrumentStore, String> {
    let Some(path) = path else {
        return parse_json_store(download_bytes(url, client).await?).await;
    };
    let Ok((temp_path, file)) = open_download_temp(path).await else {
        return parse_json_store(download_bytes(url, client).await?).await;
    };
    if let Err(err) = download_to_file(url, file, client).await {
        let _ = tokio::fs::remove_file(&temp_path).await;
        return match err {
            DownloadError::Download(message) => Err(message),
            DownloadError::CacheWrite => parse_json_store(download_bytes(url, client).await?).await,
        };
    }
    let store = match load_store_from_path(&temp_path).await {
        Ok(store) => store,
        Err(err) => {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(err);
        }
    };
    if tokio::fs::rename(&temp_path, path).await.is_err() {
        let _ = tokio::fs::remove_file(&temp_path).await;
    }
    Ok(store)
}

/// Create an exclusive sibling so concurrent downloads own separate files.
async fn open_download_temp(path: &Path) -> Result<(PathBuf, tokio::fs::File), String> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    for _ in 0..32 {
        let sequence = DOWNLOAD_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temp_path =
            path.with_extension(format!("json.{}.{sequence}.download", std::process::id()));
        match tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .await
        {
            Ok(file) => return Ok((temp_path, file)),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(err) => return Err(format!("failed to write {}: {err}", temp_path.display())),
        }
    }
    Err(format!(
        "failed to write {}: 32 temp file names were taken",
        path.display()
    ))
}

fn read_instruments_from_path(path: &Path) -> Result<Vec<Instrument>, String> {
    let file = std::fs::File::open(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    parse_instruments_reader(&mut BufReader::new(file))
}

/// # Errors
/// Returns an error if the file cannot be read or parsed, or the parse task fails.
pub async fn load_store_from_path(path: &Path) -> Result<InstrumentStore, String> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || read_instruments_from_path(&path).map(InstrumentStore::new))
        .await
        .map_err(|err| format!("instrument parse task failed: {err}"))?
}

#[must_use]
pub fn cache_path() -> Option<PathBuf> {
    let mut path = std::env::var_os("XDG_CACHE_HOME")
        .filter(|path| Path::new(path).is_absolute())
        .map(PathBuf::from)
        .or_else(dirs::cache_dir)?;
    path.push("kite");
    path.push("instruments.json");
    Some(path)
}

#[must_use]
pub fn cache_is_fresh(path: &Path) -> bool {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .is_ok_and(|modified| cache_timestamp_is_fresh(modified, SystemTime::now()))
}

/// Epoch seconds of the most recent [`PUBLISH_HOUR_IST`] that has already
/// passed, in IST. This is the instant the current day's master became
/// available, so a cache written before it is describing yesterday.
fn last_publication_cutoff(now_secs: u64) -> u64 {
    let sod = (now_secs + IST_OFFSET_SECS) % 86_400;
    let publish = PUBLISH_HOUR_IST * 3_600;
    if sod >= publish {
        now_secs - (sod - publish)
    } else {
        now_secs - (sod + 86_400 - publish)
    }
}

/// A cache is fresh from the last publication cutoff until the next one.
/// There is no maximum age because the master changes only at publication.
fn cache_timestamp_is_fresh(modified: SystemTime, now: SystemTime) -> bool {
    let epoch = |time: SystemTime| {
        time.duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs())
    };
    let now_secs = epoch(now);
    if now.duration_since(modified).is_err() {
        return false;
    }
    epoch(modified) >= last_publication_cutoff(now_secs)
}

pub async fn cache_info() -> Option<CacheInfo> {
    let path = cache_path()?;
    let meta = tokio::fs::metadata(&path).await.ok();
    let exists = meta.is_some();
    let modified = meta.as_ref().and_then(|m| m.modified().ok());
    let stale =
        !modified.is_some_and(|modified| cache_timestamp_is_fresh(modified, SystemTime::now()));
    let size_bytes = meta.as_ref().map(std::fs::Metadata::len);
    let modified_unix_seconds = modified
        .and_then(|ts| ts.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|dur| dur.as_secs());
    let record_count = if exists {
        let path = path.clone();
        tokio::task::spawn_blocking(move || {
            read_instruments_from_path(&path).map(|items| items.len())
        })
        .await
        .ok()
        .and_then(Result::ok)
    } else {
        None
    };
    Some(CacheInfo {
        path,
        exists,
        stale,
        fresh_until_unix_seconds: modified_unix_seconds
            .map(|written| last_publication_cutoff(written) + 86_400),
        size_bytes,
        modified_unix_seconds,
        record_count,
    })
}

/// # Errors
/// Returns an error if the cache file cannot be removed.
pub async fn clear_cache() -> Result<Option<PathBuf>, String> {
    let Some(path) = cache_path() else {
        return Ok(None);
    };
    match tokio::fs::remove_file(&path).await {
        Ok(()) => Ok(Some(path)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(format!("failed to remove {}: {err}", path.display())),
    }
}

/// The HTTP client for the public CDN that serves the master.
///
/// instruments.json is ~10MB compressed; use a generous timeout so a
/// slow-but-live download isn't cut short (distinct from the authed client's
/// tight 15s budget).
///
/// # Errors
/// Returns an error if the UUID header is invalid or the client cannot be built.
fn download_client(app_uuid: &str) -> Result<reqwest::Client, String> {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(
        "X-Kite-App-UUID",
        HeaderValue::from_str(app_uuid).map_err(|_| "invalid app UUID header".to_string())?,
    );
    client_builder(DOWNLOAD_TIMEOUT)
        .default_headers(headers)
        .build()
        .map_err(|err| format!("failed to build HTTP client: {err}"))
}

async fn download_response(
    url: &str,
    client: &reqwest::Client,
) -> Result<reqwest::Response, String> {
    let response = client.get(url).send().await.map_err(|err| {
        format!(
            "request failed for {url}: {:#}",
            anyhow::Error::new(err.without_url())
        )
    })?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!(
            "failed to download instruments.json: HTTP {}",
            status.as_u16()
        ));
    }
    Ok(response)
}

#[derive(Debug)]
enum DownloadError {
    Download(String),
    CacheWrite,
}

/// Stream the master into `file`, holding one response chunk at a time.
async fn download_to_file(
    url: &str,
    mut file: tokio::fs::File,
    client: &reqwest::Client,
) -> Result<(), DownloadError> {
    use tokio::io::AsyncWriteExt;

    let mut response = download_response(url, client)
        .await
        .map_err(DownloadError::Download)?;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|err| DownloadError::Download(format!("failed to read {url}: {err}")))?
    {
        file.write_all(&chunk)
            .await
            .map_err(|_| DownloadError::CacheWrite)?;
    }
    file.flush().await.map_err(|_| DownloadError::CacheWrite)?;
    Ok(())
}

/// Buffer the master when the cache cannot accept writes.
async fn download_bytes(url: &str, client: &reqwest::Client) -> Result<Vec<u8>, String> {
    let bytes = download_response(url, client)
        .await?
        .bytes()
        .await
        .map_err(|err| format!("failed to read {url}: {err}"))?;
    Ok(bytes.into())
}

async fn parse_json_store(bytes: Vec<u8>) -> Result<InstrumentStore, String> {
    tokio::task::spawn_blocking(move || {
        parse_instruments_reader(&mut Cursor::new(bytes)).map(InstrumentStore::new)
    })
    .await
    .map_err(|err| format!("instrument parse task failed: {err}"))?
}

/// Parse a JSON array or newline-delimited records without buffering the master.
fn parse_instruments_reader<R: BufRead>(reader: &mut R) -> Result<Vec<Instrument>, String> {
    let items = if opens_a_json_array(reader)? {
        serde_json::from_reader::<_, Vec<Instrument>>(reader)
            .map_err(|err| format!("invalid instruments JSON array: {err}"))?
    } else {
        let mut items = Vec::new();
        let mut line = String::new();
        let mut line_no = 0;
        loop {
            line.clear();
            if reader
                .read_line(&mut line)
                .map_err(|err| read_error(&err))?
                == 0
            {
                break;
            }
            line_no += 1;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let item = serde_json::from_str::<Instrument>(trimmed)
                .map_err(|err| format!("invalid instruments JSON line {line_no}: {err}"))?;
            items.push(item);
        }
        items
    };
    if items.is_empty() {
        return Err("instrument payload was empty".to_string());
    }
    Ok(items)
}

/// Peeks without consuming, so NDJSON line numbers stay correct.
fn opens_a_json_array<R: BufRead>(reader: &mut R) -> Result<bool, String> {
    let buffer = reader.fill_buf().map_err(|err| read_error(&err))?;
    Ok(buffer
        .iter()
        .find(|byte| !byte.is_ascii_whitespace())
        .is_some_and(|byte| *byte == b'['))
}

/// Preserve the distinct invalid-UTF-8 error from the line reader.
fn read_error(err: &std::io::Error) -> String {
    match err.kind() {
        std::io::ErrorKind::InvalidData => format!("instrument payload is not valid UTF-8: {err}"),
        _ => format!("failed to read instrument payload: {err}"),
    }
}

#[cfg(test)]
mod tests {
    use super::cache_timestamp_is_fresh;
    use std::time::{Duration, SystemTime};

    /// An IST wall-clock instant as epoch seconds. `day` is an arbitrary whole
    /// day offset so the arithmetic reads like a calendar.
    fn ist(day: u64, hour: u64, minute: u64) -> SystemTime {
        let secs = day * 86_400 + hour * 3_600 + minute * 60 - super::IST_OFFSET_SECS;
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    /// The cache is valid for exactly one publication cycle: from the 07:00 IST
    /// that produced it until the next one, however many hours that spans.
    #[test]
    fn the_cache_lives_from_one_publication_to_the_next() {
        let fetched = ist(10, 9, 0); // fetched Monday 09:00, after publication

        assert!(cache_timestamp_is_fresh(fetched, ist(10, 9, 30)));
        assert!(cache_timestamp_is_fresh(fetched, ist(10, 23, 0)));
        assert!(cache_timestamp_is_fresh(fetched, ist(11, 0, 30)));
        assert!(cache_timestamp_is_fresh(fetched, ist(11, 6, 59)));
        assert!(!cache_timestamp_is_fresh(fetched, ist(11, 7, 0)));
        assert!(!cache_timestamp_is_fresh(fetched, ist(11, 8, 0)));
    }

    /// Regression for the bug the previous rule had. Expiring at IST midnight
    /// meant a 00:30 launch refetched, got *yesterday's* master (the exchange has
    /// not published yet), stamped it with today's date and then trusted it until
    /// the next midnight, running a full day on stale data and missing every
    /// contract listed that morning.
    #[test]
    fn a_file_fetched_before_this_mornings_publication_is_stale_by_it() {
        let fetched_after_midnight = ist(10, 0, 30);
        assert!(cache_timestamp_is_fresh(
            fetched_after_midnight,
            ist(10, 6, 59)
        ));
        assert!(!cache_timestamp_is_fresh(
            fetched_after_midnight,
            ist(10, 7, 0)
        ));
        assert!(!cache_timestamp_is_fresh(
            fetched_after_midnight,
            ist(10, 15, 0)
        ));
    }

    #[test]
    fn a_cutoff_lands_on_seven_in_the_morning_ist() {
        let epoch = |t: SystemTime| t.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
        assert_eq!(
            super::last_publication_cutoff(epoch(ist(10, 7, 1))),
            epoch(ist(10, 7, 0))
        );
        assert_eq!(
            super::last_publication_cutoff(epoch(ist(10, 6, 59))),
            epoch(ist(9, 7, 0))
        );
        assert_eq!(
            super::last_publication_cutoff(epoch(ist(10, 7, 0))),
            epoch(ist(10, 7, 0))
        );
    }

    /// A file dated in the future cannot be reasoned about, so it is not trusted.
    #[test]
    fn a_future_timestamp_is_never_fresh() {
        assert!(!cache_timestamp_is_fresh(ist(11, 9, 0), ist(10, 9, 0)));
    }

    /// than `expiry`, and carrying keys [`super::Instrument`] deliberately no
    /// longer keeps (`exchange_token`, `isin`, `last_price`, `active`) which
    /// serde has to ignore.
    const ROWS: [&str; 2] = [
        r#"{"instrument_token":408065,"exchange_token":1594,"tradingsymbol":"INFY","name":"INFOSYS","exchange":"NSE","isin":"INE009A01021","last_price":0,"tick_size":0.05,"lot_size":1,"instrument_type":"EQ","segment":"NSE","active":true}"#,
        r#"{"instrument_token":13155586,"tradingsymbol":"NIFTY2671424400CE","name":"NIFTY","exchange":"NFO","strike":24400,"tick_size":0.05,"lot_size":65,"instrument_type":"CE","segment":"NFO-OPT","expiry_date":"2026-07-14"}"#,
    ];

    fn parse(payload: &str) -> Result<Vec<super::Instrument>, String> {
        super::parse_instruments_reader(&mut std::io::Cursor::new(payload.as_bytes()))
    }

    /// Every field the parse is responsible for populating, comparable across
    /// payload shapes.
    fn digest(items: &[super::Instrument]) -> Vec<String> {
        items
            .iter()
            .map(|i| serde_json::to_string(i).expect("serialize"))
            .collect()
    }

    #[test]
    fn the_streaming_parse_reads_arrays_and_newline_delimited_objects_alike() {
        let lines = parse(&format!("{}\n{}\n", ROWS[0], ROWS[1])).expect("ndjson");
        let array = parse(&format!("[{},{}]", ROWS[0], ROWS[1])).expect("array");
        assert_eq!(digest(&lines), digest(&array));

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].tradingsymbol, "INFY");
        assert_eq!(lines[1].expiry.as_deref(), Some("2026-07-14"));
        assert_eq!(lines[1].strike, Some(24400.0));

        let padded = parse(&format!("\n  {}\n\n{}\n", ROWS[0], ROWS[1])).expect("padded ndjson");
        assert_eq!(digest(&padded), digest(&lines));
    }

    #[test]
    fn a_malformed_record_is_reported_by_line_number() {
        let err = parse(&format!("{}\n{{not json}}\n", ROWS[0])).expect_err("must reject");
        assert!(err.starts_with("invalid instruments JSON line 2:"), "{err}");

        let err = parse("\n\n{not json}\n").expect_err("must reject");
        assert!(err.starts_with("invalid instruments JSON line 3:"), "{err}");

        let err = parse(&format!("[{},", ROWS[0])).expect_err("must reject");
        assert!(
            err.starts_with("invalid instruments JSON array: EOF while parsing a value"),
            "{err}"
        );
    }

    #[test]
    fn a_payload_with_no_records_is_an_error() {
        assert_eq!(parse("").unwrap_err(), "instrument payload was empty");
        assert_eq!(parse("[]").unwrap_err(), "instrument payload was empty");
        assert_eq!(
            parse("\n \n\n").unwrap_err(),
            "instrument payload was empty"
        );
    }

    #[tokio::test]
    async fn the_master_download_sends_cli_identity_headers_on_the_public_host() {
        assert_eq!(super::BASE_URL, "https://api.kite.trade");
        assert_eq!(super::INSTRUMENTS_JSON_PATH, "/instruments.json");
        let server = httpmock::MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(httpmock::Method::GET)
                    .path("/instruments.json")
                    .header("user-agent", crate::app::http::user_agent())
                    .header("x-kite-app-uuid", "5b697354-64d2-49c4-a080-6b25475555e3");
                then.status(200).body("[]");
            })
            .await;

        let response = super::download_client("5b697354-64d2-49c4-a080-6b25475555e3")
            .unwrap()
            .get(server.url("/instruments.json"))
            .send()
            .await
            .unwrap();

        mock.assert_async().await;
        assert!(response.status().is_success());
    }

    fn test_cache_path() -> std::path::PathBuf {
        let sequence = super::DOWNLOAD_SEQUENCE.fetch_add(1, super::Ordering::Relaxed);
        std::env::temp_dir()
            .join(format!(
                "kite-instruments-test-{}-{sequence}",
                std::process::id()
            ))
            .join("instruments.json")
    }

    #[tokio::test]
    async fn http_rejections_are_not_downloaded_again_and_leave_the_cache_intact() {
        let server = httpmock::MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(httpmock::Method::GET).path("/instruments.json");
                then.status(503);
            })
            .await;
        let path = test_cache_path();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, ROWS[0]).unwrap();

        let err = super::refresh_from(
            &server.url("/instruments.json"),
            Some(&path),
            &super::download_client("5b697354-64d2-49c4-a080-6b25475555e3").unwrap(),
        )
        .await
        .unwrap_err();

        assert_eq!(err, "failed to download instruments.json: HTTP 503");
        assert_eq!(mock.calls_async().await, 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), ROWS[0]);
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1
        );
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[tokio::test]
    async fn concurrent_refreshes_publish_complete_masters_and_remove_only_their_temps() {
        let server = httpmock::MockServer::start_async().await;
        let first = server
            .mock_async(|when, then| {
                when.method(httpmock::Method::GET).path("/first");
                then.status(200).body(ROWS[0]);
            })
            .await;
        let second = server
            .mock_async(|when, then| {
                when.method(httpmock::Method::GET).path("/second");
                then.status(200).body(ROWS[1]);
            })
            .await;
        let path = test_cache_path();
        let first_url = server.url("/first");
        let second_url = server.url("/second");
        let client = super::download_client("5b697354-64d2-49c4-a080-6b25475555e3").unwrap();
        let (first_store, second_store) = tokio::join!(
            super::refresh_from(&first_url, Some(&path), &client),
            super::refresh_from(&second_url, Some(&path), &client),
        );

        assert_eq!(
            first_store
                .unwrap()
                .by_token(408_065)
                .unwrap()
                .tradingsymbol,
            "INFY"
        );
        assert_eq!(
            second_store
                .unwrap()
                .by_token(13_155_586)
                .unwrap()
                .tradingsymbol,
            "NIFTY2671424400CE"
        );
        assert_eq!(first.calls_async().await, 1);
        assert_eq!(second.calls_async().await, 1);
        let cached = std::fs::read_to_string(&path).unwrap();
        assert!(cached == ROWS[0] || cached == ROWS[1], "{cached}");
        assert_eq!(super::load_store_from_path(&path).await.unwrap().len(), 1);
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1
        );
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[tokio::test]
    async fn an_unparseable_download_does_not_replace_the_cache_or_retry() {
        let server = httpmock::MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(httpmock::Method::GET).path("/instruments.json");
                then.status(200).body("[]");
            })
            .await;
        let path = test_cache_path();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, ROWS[0]).unwrap();

        assert_eq!(
            super::refresh_from(
                &server.url("/instruments.json"),
                Some(&path),
                &super::download_client("5b697354-64d2-49c4-a080-6b25475555e3").unwrap()
            )
            .await
            .unwrap_err(),
            "instrument payload was empty"
        );
        assert_eq!(mock.calls_async().await, 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), ROWS[0]);
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1
        );
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
