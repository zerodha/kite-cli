use crate::cli::{
    GttWriteArgs, JsonArrayInput, OrderModifyArgs, OrderPlaceArgs, QuoteArgs, StreamTicksArgs,
    TuiArgs,
};
use anyhow::{Context, Result, bail};
use kiteconnect::{Body, OrderParams, Request};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};
use std::fs;
use std::path::Path;

/// # Errors
/// Returns an error if no instruments are supplied.
pub fn quote_instruments(args: QuoteArgs) -> Result<Vec<String>> {
    let mut instruments = args.instruments;
    instruments.extend(args.instrument);
    if instruments.is_empty() {
        bail!("at least one instrument is required, for example NSE:INFY");
    }
    Ok(instruments)
}

#[must_use]
pub fn tui_tokens(args: TuiArgs) -> Vec<u32> {
    args.token.into_iter().chain(args.tokens).collect()
}

#[must_use]
pub fn stream_tokens(args: &StreamTicksArgs) -> Vec<u32> {
    args.token.iter().chain(&args.tokens).copied().collect()
}

/// # Errors
/// Returns an error if a value has no equals sign or an empty key.
pub fn parse_key_values(values: &[String]) -> Result<Vec<(String, String)>> {
    values
        .iter()
        .map(|item| {
            let (key, value) = item
                .split_once('=')
                .with_context(|| format!("expected KEY=VALUE, got `{item}`"))?;
            if key.trim().is_empty() {
                bail!("empty key in `{item}`");
            }
            Ok((key.to_string(), value.to_string()))
        })
        .collect()
}

/// The crate's secret keys, plus names a user may still pass to `kite request`
/// from older Kite Web sessions.
fn is_secret_key(key: &str) -> bool {
    kiteconnect::error::is_secret_key(key)
        || ["api_key", "enctoken", "public_token"]
            .iter()
            .any(|secret| key.eq_ignore_ascii_case(secret))
}

fn sanitize_query_for_display(query: Vec<(String, String)>) -> Vec<(String, String)> {
    query
        .into_iter()
        .map(|(key, value)| {
            let value = if is_secret_key(&key) {
                "<redacted>".to_string()
            } else {
                value
            };
            (key, value)
        })
        .collect()
}

fn path_for_display(path: &str) -> String {
    let Some((base, query)) = path.split_once('?') else {
        return path.to_string();
    };
    let query = query
        .split('&')
        .map(|pair| {
            if url::form_urlencoded::parse(pair.as_bytes()).any(|(key, _)| is_secret_key(&key)) {
                let key = pair.split_once('=').map_or(pair, |(key, _)| key);
                format!("{key}=<redacted>")
            } else {
                pair.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("&");
    format!("{base}?{query}")
}

fn redact_json(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                if is_secret_key(key) {
                    *value = Value::String("<redacted>".to_string());
                } else {
                    redact_json(value);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                redact_json(value);
            }
        }
        _ => {}
    }
}

#[must_use]
pub fn dry_run_value(
    method: &str,
    path: &str,
    query: Vec<(String, String)>,
    mut body: Value,
    next: &str,
) -> Value {
    redact_json(&mut body);
    json!({
        "dry_run": true,
        "method": method,
        "path": path_for_display(path),
        "query": sanitize_query_for_display(query),
        "body": body,
        "next": next,
    })
}

#[must_use]
pub fn dry_run_request(request: &Request, next: &str) -> Value {
    dry_run_value(
        request.method().as_str(),
        request.path(),
        request.query_pairs().to_vec(),
        body_for_display(request.body()),
        next,
    )
}

/// Parses a clap value the CLI restricts to Kite wire strings.
///
/// # Errors
/// Returns an error if the value is not a supported wire parameter.
pub fn wire_value<T: DeserializeOwned>(value: &str) -> kiteconnect::Result<T> {
    T::deserialize(serde::de::value::StrDeserializer::<serde::de::value::Error>::new(value))
        .map_err(|_| kiteconnect::Error::InvalidInput("invalid order parameter"))
}

/// # Errors
/// Returns an error if an order parameter is not a supported wire value.
pub fn order_place_params(args: &OrderPlaceArgs) -> kiteconnect::Result<OrderParams> {
    let mut params = OrderParams::new()
        .exchange(wire_value(&args.exchange)?)
        .tradingsymbol(&args.tradingsymbol)
        .transaction_type(wire_value(&args.transaction_type)?)
        .order_type(wire_value(&args.order_type)?)
        .quantity(args.quantity)
        .product(wire_value(&args.product)?);
    params.price = args.price;
    params.trigger_price = args.trigger_price;
    params.disclosed_quantity = args.disclosed_quantity;
    params.validity = args.validity.as_deref().map(wire_value).transpose()?;
    params.validity_ttl = args.validity_ttl;
    params.iceberg_legs = args.iceberg_legs;
    params.iceberg_quantity = args.iceberg_quantity;
    params.auction_number.clone_from(&args.auction_number);
    #[expect(
        clippy::cast_precision_loss,
        reason = "wire API represents integer market protection as f64"
    )]
    let market_protection = args.market_protection.map(|value| value as f64);
    params.market_protection = market_protection;
    params.autoslice = args.autoslice.then_some(true);
    params.tag.clone_from(&args.tag);
    Ok(params)
}

/// `None` when no order field was passed.
///
/// # Errors
/// Returns an error if an order parameter is not a supported wire value.
pub fn order_modify_params(args: &OrderModifyArgs) -> kiteconnect::Result<Option<OrderParams>> {
    let mut params = OrderParams::new();
    params.order_type = args.order_type.as_deref().map(wire_value).transpose()?;
    params.quantity = args.quantity;
    params.price = args.price;
    params.trigger_price = args.trigger_price;
    params.disclosed_quantity = args.disclosed_quantity;
    params.validity = args.validity.as_deref().map(wire_value).transpose()?;
    params.validity_ttl = args.validity_ttl;
    #[expect(
        clippy::cast_precision_loss,
        reason = "wire API represents integer market protection as f64"
    )]
    let market_protection = args.market_protection.map(|value| value as f64);
    params.market_protection = market_protection;
    let empty = params.order_type.is_none()
        && params.quantity.is_none()
        && params.price.is_none()
        && params.trigger_price.is_none()
        && params.disclosed_quantity.is_none()
        && params.validity.is_none()
        && params.validity_ttl.is_none()
        && params.market_protection.is_none();
    Ok((!empty).then_some(params))
}

/// # Errors
/// Returns an error if the source selection, file read, JSON, or array shape is invalid.
pub fn read_json_array(input: &JsonArrayInput) -> Result<Value> {
    let sources = [
        input.file.is_some(),
        input.input_json.is_some(),
        !input.order.is_empty(),
    ]
    .into_iter()
    .filter(|source| *source)
    .count();
    if sources != 1 {
        bail!("provide exactly one of --file, --input-json, or one/more --order JSON objects");
    }

    let value = if let Some(path) = &input.file {
        read_json_file(path)?
    } else if let Some(raw) = &input.input_json {
        serde_json::from_str(raw).context("invalid --input-json payload")?
    } else {
        let mut items = Vec::new();
        for raw in &input.order {
            items.push(serde_json::from_str::<Value>(raw).context("invalid --order JSON object")?);
        }
        Value::Array(items)
    };

    if !value.is_array() {
        bail!("expected a JSON array");
    }
    Ok(value)
}

/// # Errors
/// Returns an error if the JSON sources, payload shapes, or trailing parameters are invalid.
pub fn gtt_payload(args: &GttWriteArgs) -> Result<(Value, Value)> {
    let mut condition = read_one_json(
        args.condition_json.as_deref(),
        args.condition_file.as_deref(),
        "condition",
    )?;
    let orders = read_one_json(
        args.orders_json.as_deref(),
        args.orders_file.as_deref(),
        "orders",
    )?;
    let Some(object) = condition.as_object_mut() else {
        bail!("condition must be a JSON object");
    };
    if !orders.is_array() {
        bail!("orders must be a JSON array");
    }
    let trailing = args
        .trigger_type
        .as_deref()
        .unwrap_or("single")
        .starts_with("trailing-");
    if let Some(points) = args.trailing_points {
        if !trailing {
            bail!("--trailing-points requires --type trailing-single or trailing-two-leg");
        }
        if !points.is_finite() || points <= 0.0 {
            bail!("--trailing-points must be a positive number");
        }
        object.insert("trailing_points".to_string(), serde_json::json!([points]));
    }
    if trailing {
        let valid = object
            .get("trailing_points")
            .and_then(Value::as_array)
            .is_some_and(|values| {
                values.len() == 1
                    && values[0]
                        .as_f64()
                        .is_some_and(|points| points.is_finite() && points > 0.0)
            });
        if !valid {
            bail!(
                "trailing GTT requires --trailing-points or condition.trailing_points with one positive value"
            );
        }
    } else if object.get("trailing_points").is_some() {
        bail!("condition.trailing_points requires a trailing GTT type");
    }
    Ok((condition, orders))
}

/// # Errors
/// Returns an error if body sources conflict or the selected body cannot be read or parsed.
pub fn raw_body(args: &crate::cli::RawWriteArgs) -> Result<Body> {
    let sources = [
        args.json_body.is_some(),
        args.json_file.is_some(),
        !args.form.is_empty(),
    ]
    .into_iter()
    .filter(|source| *source)
    .count();

    if sources > 1 {
        bail!("provide only one body source: --body-json, --body-file, or --form");
    }

    if let Some(raw) = &args.json_body {
        return Ok(Body::Json(
            serde_json::from_str(raw).context("invalid --body-json body")?,
        ));
    }
    if let Some(path) = &args.json_file {
        return Ok(Body::Json(read_json_file(path)?));
    }
    if !args.form.is_empty() {
        return Ok(Body::Form(parse_key_values(&args.form)?));
    }
    Ok(Body::None)
}

#[must_use]
pub fn body_for_display(body: &Body) -> Value {
    match body {
        Body::Form(form) => {
            let form: Map<String, Value> = form
                .iter()
                .map(|(key, value)| (key.clone(), Value::String(value.clone())))
                .collect();
            json!({ "form": form })
        }
        Body::Json(value) => json!({ "json": value }),
        _ => Value::Null,
    }
}

/// # Errors
/// Returns an error if CSV headers or a record cannot be parsed.
pub fn csv_records(csv_text: &str, limit: usize) -> Result<(Vec<Value>, bool)> {
    let mut reader = csv::Reader::from_reader(csv_text.as_bytes());
    let headers: Vec<String> = reader
        .headers()
        .context("CSV missing headers")?
        .iter()
        .map(ToString::to_string)
        .collect();
    let mut records = Vec::new();
    let mut truncated = false;
    for record in reader.records() {
        let record = record.context("failed to parse CSV record")?;
        if limit > 0 && records.len() >= limit {
            truncated = true;
            break;
        }
        let mut object = Map::new();
        for (idx, value) in record.iter().enumerate() {
            if let Some(key) = headers.get(idx) {
                object.insert(key.clone(), Value::String(value.to_string()));
            }
        }
        records.push(Value::Object(object));
    }
    Ok((records, truncated))
}

/// # Errors
/// Returns an error if CSV headers or a record cannot be parsed.
pub fn search_csv(csv_text: &str, query: &str, limit: usize) -> Result<Vec<Value>> {
    let mut reader = csv::Reader::from_reader(csv_text.as_bytes());
    let headers = reader.headers().context("CSV missing headers")?.clone();
    let columns: Vec<usize> = ["tradingsymbol", "name", "instrument_token", "exchange"]
        .iter()
        .filter_map(|field| headers.iter().position(|header| header == *field))
        .collect();
    let needle = query.to_ascii_lowercase();
    let mut matches = Vec::new();
    for record in reader.records() {
        let record = record.context("failed to parse CSV record")?;
        if (limit == 0 || matches.len() < limit)
            && columns
                .iter()
                .any(|&index| record[index].to_ascii_lowercase().contains(&needle))
        {
            matches.push(Value::Object(
                headers
                    .iter()
                    .zip(record.iter())
                    .map(|(key, value)| (key.to_string(), Value::String(value.to_string())))
                    .collect(),
            ));
        }
    }
    Ok(matches)
}

/// The element after `current`, wrapping to the first element.
/// An absent current selects the first element; an empty list keeps current.
pub fn cycle_next<T: Copy + PartialEq>(all: &[T], current: T) -> T {
    match all.iter().position(|item| *item == current) {
        Some(idx) => all[(idx + 1) % all.len()],
        None => all.first().copied().unwrap_or(current),
    }
}

/// Serialize `value` as pretty JSON and replace `path` with it atomically,
/// creating the parent directory if it does not exist yet (a fresh
/// kite config directory). The shape every config writer in the TUI wants.
///
/// # Errors
/// Returns an error if serialization or the atomic file replacement fails.
pub fn save_json_atomic<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let raw = serde_json::to_string_pretty(value).map_err(|err| err.to_string())? + "\n";
    crate::app::runtime::replace_private_file(path, raw.as_bytes(), || {
        crate::app::runtime::CONFIG_TMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    })
    .map_err(|err| err.to_string())
}

fn read_json_file(path: &Path) -> Result<Value> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read JSON file {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("invalid JSON in {}", path.display()))
}

fn read_one_json(inline: Option<&str>, file: Option<&Path>, name: &str) -> Result<Value> {
    match (inline, file) {
        (Some(_), Some(_)) => bail!("provide either --{name}-json or --{name}-file, not both"),
        (Some(raw), None) => {
            serde_json::from_str(raw).with_context(|| format!("invalid {name} JSON"))
        }
        (None, Some(path)) => read_json_file(path),
        (None, None) => bail!("missing --{name}-json or --{name}-file"),
    }
}

#[cfg(test)]
mod tests {
    use super::{csv_records, gtt_payload, parse_key_values};
    use crate::cli::{ConfirmArgs, GttWriteArgs};

    fn gtt_args(trigger_type: &str, trailing_points: Option<f64>) -> GttWriteArgs {
        GttWriteArgs {
            trigger_type: Some(trigger_type.to_string()),
            trailing_points,
            condition_json: Some(
                r#"{"exchange":"NSE","tradingsymbol":"INFY","trigger_values":[100],"last_price":110}"#
                    .to_string(),
            ),
            condition_file: None,
            orders_json: Some("[]".to_string()),
            orders_file: None,
            confirm: ConfirmArgs {
                dry_run: true,
                confirm: false,
            },
        }
    }

    /// The one behaviour every `cycle()` key depends on: forward through the
    /// list, wrapping, and never stalling or skipping.
    #[test]
    fn cycle_next_advances_and_wraps() {
        let all = ['a', 'b', 'c'];
        assert_eq!(super::cycle_next(&all, 'a'), 'b');
        assert_eq!(super::cycle_next(&all, 'b'), 'c');
        assert_eq!(super::cycle_next(&all, 'c'), 'a');
        let mut seen = Vec::new();
        let mut current = 'a';
        for _ in 0..all.len() {
            seen.push(current);
            current = super::cycle_next(&all, current);
        }
        assert_eq!(seen, all);
        assert_eq!(current, 'a');
        assert_eq!(super::cycle_next(&all, 'z'), 'a');
        assert_eq!(super::cycle_next(&[], 'z'), 'z');
        assert_eq!(super::cycle_next(&['x'], 'x'), 'x');
    }

    #[test]
    fn parses_key_values() {
        let pairs = parse_key_values(&["a=b".to_string(), "x=1=2".to_string()]).unwrap();
        assert_eq!(pairs[0], ("a".to_string(), "b".to_string()));
        assert_eq!(pairs[1], ("x".to_string(), "1=2".to_string()));
    }

    #[test]
    fn parses_csv_records_with_limit() {
        let (records, truncated) = csv_records("a,b\n1,2\n3,4\n", 1).unwrap();
        assert_eq!(records.len(), 1);
        assert!(truncated);
    }

    #[test]
    fn trailing_gtt_inserts_points_into_condition() {
        let (condition, _) = gtt_payload(&gtt_args("trailing-single", Some(2.0))).unwrap();
        assert_eq!(condition["trailing_points"], serde_json::json!([2.0]));
    }

    #[test]
    fn trailing_gtt_requires_positive_points() {
        assert!(gtt_payload(&gtt_args("trailing-two-leg", None)).is_err());
        assert!(gtt_payload(&gtt_args("trailing-single", Some(0.0))).is_err());
        assert!(gtt_payload(&gtt_args("single", Some(2.0))).is_err());
    }
    #[test]
    fn dry_runs_redact_every_secret_without_changing_the_request() {
        use kiteconnect::{Body, Request};
        use serde_json::json;
        for key in [
            "password",
            "api_secret",
            "checksum",
            "twofa_value",
            "access_token",
            "refresh_token",
            "request_token",
            "api_key",
        ] {
            let key = key.to_ascii_uppercase();
            let encoded_key = format!("%{:02X}{}", key.as_bytes()[0], &key[1..]);
            let path = format!("/audit?{encoded_key}=PATH_SENTINEL&safe=a%20b#fragment");
            let query = vec![
                (key.clone(), "QUERY_SENTINEL".to_string()),
                ("safe".to_string(), "keep".to_string()),
            ];
            let form = vec![
                (key.clone(), "FORM_SENTINEL".to_string()),
                ("safe".to_string(), "keep".to_string()),
            ];
            let request = Request::post(&path).query(query.clone()).form(form.clone());
            let display = super::dry_run_request(&request, "confirm");
            assert_eq!(
                display["path"],
                format!("/audit?{encoded_key}=<redacted>&safe=a%20b#fragment")
            );
            assert_eq!(
                display["query"],
                json!([[key, "<redacted>"], ["safe", "keep"]])
            );
            assert_eq!(
                display["body"]["form"],
                json!({ &key: "<redacted>", "safe":"keep" })
            );
            let output = display.to_string();
            for sentinel in ["PATH_SENTINEL", "QUERY_SENTINEL", "FORM_SENTINEL"] {
                assert!(!output.contains(sentinel));
            }
            assert_eq!(request.path(), path);
            assert_eq!(request.query_pairs(), query);
            assert!(matches!(request.body(), Body::Form(values) if *values == form));

            let body = json!({ "nested": [{ &key: {"value":"JSON_SENTINEL"}, "safe":"keep" }] });
            let request = Request::put("/audit").json(body.clone());
            let display = super::dry_run_request(&request, "confirm");
            assert_eq!(
                display["body"]["json"],
                json!({"nested":[{ &key: "<redacted>", "safe":"keep" }]})
            );
            assert!(!display.to_string().contains("JSON_SENTINEL"));
            assert!(matches!(request.body(), Body::Json(value) if *value == body));
        }
    }

    #[test]
    fn dry_runs_keep_instrument_tokens_and_redact_secrets_after_a_hash() {
        use kiteconnect::Request;
        use serde_json::json;
        let request = Request::post("/margins/orders?instrument_token=408065")
            .query([("instrument_token", "408065")])
            .form([("instrument_token", "408065")]);
        let display = super::dry_run_request(&request, "confirm");
        assert_eq!(display["path"], "/margins/orders?instrument_token=408065");
        assert_eq!(display["query"], json!([["instrument_token", "408065"]]));
        assert_eq!(
            display["body"]["form"],
            json!({"instrument_token":"408065"})
        );
        let body = json!([{"tradingsymbol":"INFY", "instrument_token":408_065}]);
        let display =
            super::dry_run_request(&Request::post("/margins/orders").json(body.clone()), "");
        assert_eq!(display["body"]["json"], body);

        let request = Request::delete("/x?foo=1#&access_token=PATH_SENTINEL");
        let display = super::dry_run_request(&request, "confirm");
        assert_eq!(display["path"], "/x?foo=1#&access_token=<redacted>");
        assert!(!display.to_string().contains("PATH_SENTINEL"));

        let request =
            Request::get("/x").query([("enctoken", "E_SENTINEL"), ("public_token", "P_SENTINEL")]);
        let display = super::dry_run_request(&request, "confirm").to_string();
        assert!(!display.contains("E_SENTINEL") && !display.contains("P_SENTINEL"));
    }

    #[test]
    fn csv_search_preserves_matching_limits_headers_and_errors() {
        use serde_json::json;
        let csv = "tradingsymbol,name,instrument_token,exchange,extra\nABC,First Fund,123,NSE,x\nDEF,Second Fund,456,BSE,y\n";
        assert_eq!(
            super::search_csv(csv, "fUnD", 1).unwrap(),
            vec![
                json!({"tradingsymbol":"ABC","name":"First Fund","instrument_token":"123","exchange":"NSE","extra":"x"})
            ]
        );
        assert_eq!(
            super::search_csv(csv, "456", 5).unwrap()[0]["tradingsymbol"],
            "DEF"
        );
        assert_eq!(
            super::search_csv(csv, "bse", 5).unwrap()[0]["tradingsymbol"],
            "DEF"
        );
        assert_eq!(
            super::search_csv(csv, "abc", 5).unwrap()[0]["name"],
            "First Fund"
        );
        assert_eq!(super::search_csv(csv, "x", 5).unwrap().len(), 0);
        assert_eq!(super::search_csv(csv, "", 0).unwrap().len(), 2);
        assert_eq!(super::search_csv(csv, "", 5).unwrap().len(), 2);
        let malformed = format!("{csv}bad,row\n");
        assert!(super::search_csv(&malformed, "fund", 1).is_err());
        assert!(super::search_csv(&malformed, "", 0).is_err());
    }

    #[test]
    fn wire_values_accept_each_cli_enum_and_reject_invalid_input() {
        use kiteconnect::{
            Exchange, OrderType, PositionType, Product, TransactionType, Validity, Variety,
        };
        fn check<T: serde::de::DeserializeOwned + std::fmt::Display>(values: &[&str]) {
            for value in values {
                assert_eq!(super::wire_value::<T>(value).unwrap().to_string(), *value);
            }
            assert_eq!(
                super::wire_value::<T>("invalid").err().unwrap().to_string(),
                "invalid order parameter"
            );
        }
        check::<Exchange>(&["NSE", "BSE", "NFO", "BFO", "CDS", "BCD", "MCX", "NCO"]);
        check::<TransactionType>(&["BUY", "SELL"]);
        check::<Product>(&["CNC", "NRML", "MIS", "MTF", "CO", "BO"]);
        check::<OrderType>(&["MARKET", "LIMIT", "SL", "SL-M"]);
        check::<Validity>(&["DAY", "IOC", "TTL"]);
        check::<Variety>(&["regular", "amo", "co", "iceberg", "auction"]);
        check::<PositionType>(&["day", "overnight"]);
    }

    #[test]
    fn concurrent_json_saves_publish_complete_values_without_temporary_files() {
        let dir = std::env::temp_dir().join(format!("kite-util-concurrent-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("notes.json");
        let values: Vec<_> = (0..16)
            .map(|index| serde_json::json!({"index": index, "notes":"x".repeat(8192)}))
            .collect();
        let barrier = std::sync::Barrier::new(values.len());
        std::thread::scope(|scope| {
            for value in &values {
                let path = &path;
                let barrier = &barrier;
                scope.spawn(move || {
                    barrier.wait();
                    for _ in 0..8 {
                        super::save_json_atomic(path, value).unwrap();
                    }
                });
            }
        });
        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert!(values.contains(&saved));
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn failed_json_save_leaves_no_temp_file() {
        let dir =
            std::env::temp_dir().join(format!("kite-util-failed-save-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("notes.json");
        std::fs::create_dir(&path).unwrap();
        assert!(super::save_json_atomic(&path, &serde_json::json!({"notes":[]})).is_err());
        assert!(path.is_dir());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn csv_search_zero_limit_returns_all_matches() {
        let csv = "tradingsymbol,name\nINFY,Infosys\nINF2,Another fund\nTCS,Tata\n";
        let matches = super::search_csv(csv, "INF", 0).unwrap();
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0]["tradingsymbol"], "INFY");
        assert_eq!(matches[1]["tradingsymbol"], "INF2");
        assert_eq!(super::search_csv(csv, "INF", 1).unwrap().len(), 1);
    }
}
