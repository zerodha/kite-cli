use httpmock::prelude::*;
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Setup {
    home: PathBuf,
    config: PathBuf,
    server: MockServer,
}

impl Setup {
    fn new() -> Self {
        let home = std::env::temp_dir().join(format!(
            "kite-style-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&home).unwrap();
        let setup = Self {
            config: home.join("config.json"),
            home,
            server: MockServer::start(),
        };
        setup.save_session();
        setup
    }

    fn save_session(&self) {
        fs::write(
            &self.config,
            serde_json::to_vec(&json!({
                "api_root": self.server.base_url(), "ticker_url": "ws://fixture.invalid",
                "api_key": "test-key", "access_token": "test-token", "user_id": "AB1234",
                "app_uuid": "00000000-0000-4000-8000-000000000000"
            }))
            .unwrap(),
        )
        .unwrap();
    }

    fn run(&self, binary: &Path, args: &[&str], json: bool) -> Output {
        let mut command = Command::new(binary);
        self.configure_command(&mut command)
            .arg("--config")
            .arg(&self.config);
        if json {
            command.arg("--json");
        }
        command.args(args).output().unwrap()
    }

    fn configure_command<'a>(&self, command: &'a mut Command) -> &'a mut Command {
        isolated_env(command)
            .env("HOME", &self.home)
            .env("XDG_CONFIG_HOME", &self.home)
            .env("XDG_CACHE_HOME", &self.home)
            .env("XDG_DATA_HOME", &self.home)
            .env("XDG_STATE_HOME", &self.home)
            .env("APPDATA", &self.home)
            .env("LOCALAPPDATA", &self.home)
            .env("USERPROFILE", &self.home)
            .env("CARGO_HOME", &self.home)
            .env("RUSTUP_HOME", &self.home)
            .env("MISE_DATA_DIR", &self.home)
            .env("TEMP", &self.home)
            .env("TMP", &self.home)
            .env("NO_COLOR", "")
    }

    fn cache_path(&self) -> PathBuf {
        self.home.join("kite").join("instruments.json")
    }

    fn normalized(&self, bytes: &[u8]) -> String {
        let cache = self.cache_path();
        let encoded_cache = serde_json::to_string(&cache).unwrap();
        let encoded_home = serde_json::to_string(&self.home).unwrap();
        String::from_utf8(bytes.to_vec())
            .unwrap()
            .replace(&self.server.base_url(), "http://fixture.invalid")
            .replace(
                encoded_cache.trim_matches('"'),
                "/home/user/kite/instruments.json",
            )
            .replace(cache.to_str().unwrap(), "/home/user/kite/instruments.json")
            .replace(encoded_home.trim_matches('"'), "/home/user")
            .replace(self.home.to_str().unwrap(), "/home/user")
            .replace("/home/user\\\\", "/home/user/")
            .replace("/home/user\\", "/home/user/")
    }
}

fn isolated_env(command: &mut Command) -> &mut Command {
    command.env_clear();
    #[cfg(windows)]
    for name in ["SYSTEMROOT"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    command
}

impl Drop for Setup {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.home).unwrap();
    }
}

fn fixture(name: &str) -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("mock_responses")
            .join(name),
    )
    .unwrap()
}

fn golden(name: &str, stream: &str) -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("cli-output")
            .join(format!("{name}.{stream}")),
    )
    .unwrap()
    .replace("0.1.0", env!("CARGO_PKG_VERSION"))
}

const PLACE: &[&str] = &[
    "orders",
    "place",
    "--exchange",
    "NSE",
    "--tradingsymbol",
    "INFY",
    "--transaction-type",
    "BUY",
    "--order-type",
    "LIMIT",
    "--quantity",
    "1",
    "--product",
    "CNC",
    "--price",
    "1500",
    "--validity",
    "DAY",
];

#[test]
fn human_fixture_goldens_and_json_bytes() {
    let cases: &[(&str, &[&str], &str, &str)] = &[
        (
            "holdings",
            &["portfolio", "holdings"],
            "/portfolio/holdings",
            "holdings.json",
        ),
        (
            "positions",
            &["portfolio", "positions"],
            "/portfolio/positions",
            "positions.json",
        ),
        ("orders", &["orders", "list"], "/orders", "orders.json"),
        (
            "ltp",
            &["market", "ltp", "NSE:INFY", "NSE:MISSING"],
            "/quote/ltp",
            "ltp.json",
        ),
        (
            "quote",
            &["market", "quote", "NSE:INFY"],
            "/quote",
            "quote.json",
        ),
        (
            "profile",
            &["user", "profile"],
            "/user/profile",
            "profile.json",
        ),
        (
            "margins",
            &["user", "margins"],
            "/user/margins",
            "margins.json",
        ),
        (
            "gtt",
            &["gtt", "list"],
            "/gtt/triggers",
            "gtt_get_orders.json",
        ),
    ];
    for &(name, args, endpoint, file) in cases {
        let setup = Setup::new();
        let body = fixture(file);
        let mock = setup.server.mock(|when, then| {
            when.method(GET).path(endpoint);
            then.status(200).body(&body);
        });
        let human = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), args, false);
        assert_eq!(human.status.code(), Some(0), "{name}");
        assert_golden(&setup, name, &human);
        let wire = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), args, true);
        let value: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            wire.stdout,
            format!(
                "{}\n",
                serde_json::to_string_pretty(&value["data"]).unwrap()
            )
            .as_bytes(),
            "{name}"
        );
        assert_eq!(wire.stderr, b"", "{name}");
        assert_json_golden(&setup, &format!("json-{name}"), &wire);
        mock.assert_calls(2);
    }
}

fn assert_golden(setup: &Setup, name: &str, output: &Output) {
    for (stream, bytes) in [("stdout", &output.stdout), ("stderr", &output.stderr)] {
        let actual = setup.normalized(bytes);
        assert_eq!(actual, golden(name, stream), "{name} {stream}");
        assert!(!actual.contains('\u{1b}'), "{name} {stream}");
    }
}

fn assert_json_golden(setup: &Setup, name: &str, output: &Output) {
    assert_golden(setup, name, output);
    let expected = golden(name, "exit").trim().parse::<i32>().unwrap();
    assert_eq!(output.status.code(), Some(expected), "{name} exit");
}

#[test]
fn write_empty_and_error_streams_have_literal_outputs_and_exit_codes() {
    let setup = Setup::new();
    let dry = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), PLACE, false);
    assert_eq!(dry.status.code(), Some(0));
    assert_golden(&setup, "dry-run", &dry);
    let wire = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), PLACE, true);
    assert_json_golden(&setup, "json-place-dry-run", &wire);
    let mock = setup.server.mock(|when, then| {
        when.method(POST).path("/orders/regular");
        then.status(200).body(fixture("order_response.json"));
    });
    let mut args = PLACE.to_vec();
    args.push("--confirm");
    let live = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &args, false);
    assert_eq!(live.status.code(), Some(0));
    assert_golden(&setup, "submitted", &live);
    let wire = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &args, true);
    assert_eq!(wire.stdout, b"{\n  \"order_id\": \"151220000000000\"\n}\n");
    assert_json_golden(&setup, "json-place-submitted", &wire);
    mock.assert_calls(2);

    let setup = Setup::new();
    setup.server.mock(|when, then| {
        when.method(GET).path("/orders");
        then.status(200).body(fixture("orders.json"));
    });
    let empty = setup.run(
        Path::new(env!("CARGO_BIN_EXE_kite")),
        &["orders", "list", "--status", "OPEN"],
        false,
    );
    assert_eq!(empty.status.code(), Some(0));
    assert_golden(&setup, "empty", &empty);
    assert_eq!(empty.stdout, b"");

    let setup = Setup::new();
    setup.server.mock(|when, then| {
        when.method(GET).path("/portfolio/holdings");
        then.status(403).json_body(
            json!({"status":"error","error_type":"TokenException","message":"Invalid session"}),
        );
    });
    let args = ["portfolio", "holdings"];
    let expired = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &args, false);
    assert_eq!(expired.status.code(), Some(1));
    assert_eq!(expired.stdout, b"");
    assert_golden(&setup, "expired", &expired);
    let wire = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &args, true);
    assert_eq!(wire.stdout, b"{\n  \"error\": {\n    \"message\": \"session expired; run `kite auth login`\",\n    \"type\": \"Error\"\n  },\n  \"ok\": false\n}\n");
    assert_json_golden(&setup, "json-expired", &wire);
}

#[test]
fn logout_is_immediate_and_preserves_live_json_on_both_outcomes() {
    for (name, status, body) in [
        ("logout", 200, json!({"status":"success","data":true})),
        (
            "logout-failed",
            403,
            json!({"status":"error","error_type":"InputException","message":"revoke failed"}),
        ),
    ] {
        let setup = Setup::new();
        let mock = setup.server.mock(|when, then| {
            when.method(DELETE).path("/session/token");
            then.status(status).json_body(body);
        });
        let human = setup.run(
            Path::new(env!("CARGO_BIN_EXE_kite")),
            &["auth", "logout"],
            false,
        );
        assert_eq!(human.status.code(), Some(0));
        assert_golden(&setup, name, &human);
        let config: Value = serde_json::from_slice(&fs::read(&setup.config).unwrap()).unwrap();
        assert_eq!(config["access_token"], Value::Null);
        assert_eq!(config["user_id"], "AB1234");
        setup.save_session();
        let wire = setup.run(
            Path::new(env!("CARGO_BIN_EXE_kite")),
            &["auth", "logout"],
            true,
        );
        assert_eq!(wire.status.code(), Some(0));
        setup.save_session();
        assert_json_golden(&setup, &format!("json-{name}"), &wire);
        mock.assert_calls(2);
        setup.save_session();
        let confirmed = setup.run(
            Path::new(env!("CARGO_BIN_EXE_kite")),
            &["auth", "logout", "--confirm"],
            false,
        );
        assert_eq!(confirmed.status.code(), human.status.code());
        assert_eq!(confirmed.stdout, human.stdout);
        assert_eq!(confirmed.stderr, human.stderr);
        let cleared: Value = serde_json::from_slice(&fs::read(&setup.config).unwrap()).unwrap();
        assert_eq!(cleared["access_token"], Value::Null);
        mock.assert_calls(3);
        setup.save_session();
        let saved = fs::read(&setup.config).unwrap();
        let rejected = setup.run(
            Path::new(env!("CARGO_BIN_EXE_kite")),
            &["auth", "logout", "--dry-run"],
            false,
        );
        assert_eq!(rejected.status.code(), Some(2));
        assert_eq!(rejected.stdout, b"");
        assert_eq!(fs::read(&setup.config).unwrap(), saved);
        mock.assert_calls(3);
    }
}

#[test]
fn doctor_and_paths_are_human_and_json_stays_unchanged() {
    let setup = Setup::new();
    let args = ["doctor", "--no-live"];
    let human = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &args, false);
    assert_eq!(human.status.code(), Some(0));
    assert_golden(&setup, "doctor", &human);
    for (name, args) in [
        ("json-doctor-no-live", &args[..]),
        ("json-config-path", &["config", "path"]),
        ("json-config-show", &["config", "show"]),
    ] {
        let wire = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), args, true);
        assert_json_golden(&setup, name, &wire);
    }
    let path = setup.run(
        Path::new(env!("CARGO_BIN_EXE_kite")),
        &["config", "path"],
        false,
    );
    assert_eq!(
        path.stdout,
        format!("{}\n", setup.config.display()).as_bytes()
    );
    assert_eq!(path.stderr, b"");
}

#[test]
fn doctor_reports_failed_checks_without_changing_output_format() {
    let setup = Setup::new();
    let mock = setup.server.mock(|when, then| {
        when.method(GET).path("/user/margins");
        then.status(500).json_body(json!({
            "status": "error", "message": "fixture unavailable", "error_type": "GeneralException"
        }));
    });
    for json_mode in [false, true] {
        let output = setup.run(
            Path::new(env!("CARGO_BIN_EXE_kite")),
            &["doctor"],
            json_mode,
        );
        assert_eq!(output.status.code(), Some(1));
        if json_mode {
            let value: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(value["ok"], false);
            assert_eq!(value["api"]["ok"], false);
            assert!(value.get("error").is_none());
            assert_eq!(
                output.stdout,
                format!("{}\n", serde_json::to_string_pretty(&value).unwrap()).as_bytes()
            );
            assert_eq!(output.stderr, b"");
        }
    }
    mock.assert_calls(2);
    fs::write(
        &setup.config,
        json!({
            "api_root": setup.server.base_url(), "api_key": "test-key"
        })
        .to_string(),
    )
    .unwrap();
    let output = setup.run(
        Path::new(env!("CARGO_BIN_EXE_kite")),
        &["doctor", "--no-live"],
        true,
    );
    assert_eq!(output.status.code(), Some(1));
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["setup"], json!(["run `kite auth login`"]));
}

#[test]
fn doctor_names_missing_configuration_before_suggesting_login() {
    let setup = Setup::new();
    for (config, expected) in [
        (
            json!({}),
            json!([
                "no API root configured; set KITE_API_ROOT or \"api_root\" in config.json",
                "no API key configured; set KITE_API_KEY or \"api_key\" in config.json"
            ]),
        ),
        (
            json!({"api_root": setup.server.base_url()}),
            json!(["no API key configured; set KITE_API_KEY or \"api_key\" in config.json"]),
        ),
        (
            json!({"api_key": "test-key"}),
            json!(["no API root configured; set KITE_API_ROOT or \"api_root\" in config.json"]),
        ),
    ] {
        fs::write(&setup.config, config.to_string()).unwrap();
        let output = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &["doctor"], true);
        assert_eq!(output.status.code(), Some(1));
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["ok"], false);
        assert_eq!(value["setup"], expected);
        assert_eq!(value["api"]["reason"], "not configured");
    }
}

#[test]
fn help_uses_platform_neutral_config_path() {
    let setup = Setup::new();
    for args in [&["--help"][..], &["config", "--help"][..]] {
        let output = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), args, false);
        assert_eq!(output.status.code(), Some(0));
        let help = String::from_utf8(output.stdout).unwrap();
        assert!(help.contains("kite config path"));
        assert!(!help.contains("~/.config/kite/config.json"));
    }
}

#[test]
fn unreachable_ticker_fails_instead_of_reporting_zero_ticks() {
    let setup = Setup::new();
    let address = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    fs::write(
        &setup.config,
        json!({
            "api_root": setup.server.base_url(), "api_key": "test-key",
            "access_token": "test-token", "ticker_url": format!("ws://{address}")
        })
        .to_string(),
    )
    .unwrap();
    for json_mode in [false, true] {
        let output = setup.run(
            Path::new(env!("CARGO_BIN_EXE_kite")),
            &[
                "stream",
                "ticks",
                "--token",
                "123",
                "--timeout-seconds",
                "1",
            ],
            json_mode,
        );
        assert_eq!(output.status.code(), Some(1));
        let message = if json_mode {
            let value: Value = serde_json::from_slice(&output.stdout).unwrap();
            value["error"]["message"].as_str().unwrap().to_string()
        } else {
            String::from_utf8(output.stderr).unwrap()
        };
        assert!(message.contains("ticker never connected:"), "{message}");
        assert!(!message.contains("0 ticks in"));
    }
}

const OTHER_CASES: &[(&[&str], &str, &str, &str)] = &[
    (
        &["market", "ohlc", "NSE:INFY"],
        "GET",
        "/quote/ohlc",
        "ohlc.json",
    ),
    (
        &["quotes", "ltp", "NSE:INFY"],
        "GET",
        "/quote/ltp",
        "ltp.json",
    ),
    (
        &["orders", "get", "123"],
        "GET",
        "/orders/123",
        "order_info.json",
    ),
    (&["orders", "trades"], "GET", "/trades", "trades.json"),
    (
        &["orders", "trades", "--order-id", "123"],
        "GET",
        "/orders/123/trades",
        "order_trades.json",
    ),
    (
        &["portfolio", "auctions"],
        "GET",
        "/portfolio/holdings/auctions",
        "auctions_list.json",
    ),
    (
        &["gtt", "get", "123"],
        "GET",
        "/gtt/triggers/123",
        "gtt_get_order.json",
    ),
    (&["mf", "orders"], "GET", "/mf/orders", "mf_orders.json"),
    (
        &["mf", "order", "123"],
        "GET",
        "/mf/orders/123",
        "mf_orders_info.json",
    ),
    (&["mf", "sips"], "GET", "/mf/sips", "mf_sips.json"),
    (
        &["mf", "holdings"],
        "GET",
        "/mf/holdings",
        "mf_holdings.json",
    ),
    (
        &["user", "margins", "--segment", "equity"],
        "GET",
        "/user/margins/equity",
        "margins_equity.json",
    ),
    (
        &["margins", "orders", "--input-json", "[]"],
        "POST",
        "/margins/orders",
        "order_margins.json",
    ),
    (
        &["margins", "basket", "--input-json", "[]"],
        "POST",
        "/margins/basket",
        "basket_margins.json",
    ),
    (
        &["margins", "charges", "--input-json", "[]"],
        "POST",
        "/charges/orders",
        "virtual_contract_note.json",
    ),
    (
        &[
            "historical",
            "candles",
            "--instrument-token",
            "123",
            "--interval",
            "minute",
            "--from",
            "2021-01-01",
            "--to",
            "2021-01-02",
        ],
        "GET",
        "/instruments/historical/123/minute",
        "historical_minute.json",
    ),
];

#[test]
fn other_command_families_preserve_fixture_json_bytes() {
    for &(args, method, endpoint, file) in OTHER_CASES {
        let setup = Setup::new();
        let mut envelope: Value = serde_json::from_str(&fixture(file)).unwrap();
        envelope["status"] = json!("success");
        let body = envelope.to_string();
        setup.server.mock(|when, then| {
            when.method(method).path(endpoint);
            then.status(200).body(&body);
        });
        let wire = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), args, true);
        assert_eq!(
            wire.status.code(),
            Some(0),
            "{args:?}: {}",
            String::from_utf8_lossy(&wire.stderr)
        );
        let value: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            wire.stdout,
            format!(
                "{}\n",
                serde_json::to_string_pretty(&value["data"]).unwrap()
            )
            .as_bytes(),
            "{args:?}"
        );
        assert_json_golden(
            &setup,
            &format!("json-{}", file.trim_end_matches(".json")),
            &wire,
        );
        let human = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), args, false);
        assert_eq!(human.status.code(), Some(0), "{args:?}");
        assert!(!human.stdout.starts_with(b"{"), "{args:?}");
        assert!(!human.stdout.contains(&0x1b), "{args:?}");
        let text = String::from_utf8(human.stdout).unwrap();
        let expected: &[&str] = match file {
            "trades.json" => &["AVG", "420.65"],
            "order_trades.json" => &["AVG", "4,852.00"],
            "mf_holdings.json" => &["QTY", "1,874.10", "84.86"],
            "mf_orders.json" => &["AMOUNT", "1,000.00", "Insufficient balance"],
            "mf_orders_info.json" => &["5,000.00", "10.43"],
            "mf_sips.json" => &["INSTALMENT AMOUNT", "500.00"],
            "auctions_list.json" => &["AUCTION NUMBER", "131.95", "142.50"],
            "order_margins.json" => &[
                "charges.transaction_tax",
                "1.50",
                "charges.gst.total",
                "0.01",
                "pnl.realised",
                "pnl.unrealised",
            ],
            "basket_margins.json" => &[
                "initial.total",
                "96504.98",
                "final.total",
                "34786.73",
                "orders[0].charges.total",
                "26.85",
                "charges.brokerage",
                "40",
            ],
            "virtual_contract_note.json" => &[
                "charges.transaction_tax",
                "0.56",
                "charges.gst.total",
                "0.00",
                "charges.total",
                "0.58",
            ],
            "historical_minute.json" => {
                &["TIME", "OPEN", "HIGH", "LOW", "CLOSE", "VOLUME", "1,704.50"]
            }
            _ => &[],
        };
        for field in expected {
            assert!(text.contains(field), "{args:?} missing {field}: {text}");
        }
    }
}

const WRITE_CASES: &[(&[&str], &str, &str, &str)] = &[
    (
        &[
            "orders",
            "modify",
            "--order-id",
            "123",
            "--quantity",
            "2",
            "--confirm",
        ],
        "PUT",
        "/orders/regular/123",
        "order_modify.json",
    ),
    (
        &["orders", "cancel", "--order-id", "123", "--confirm"],
        "DELETE",
        "/orders/regular/123",
        "order_cancel.json",
    ),
    (
        &[
            "gtt",
            "place",
            "--condition-json",
            "{}",
            "--orders-json",
            "[]",
            "--confirm",
        ],
        "POST",
        "/gtt/triggers",
        "gtt_place_order.json",
    ),
    (
        &[
            "gtt",
            "modify",
            "--trigger-id",
            "123",
            "--type",
            "single",
            "--condition-json",
            "{}",
            "--orders-json",
            "[]",
            "--confirm",
        ],
        "PUT",
        "/gtt/triggers/123",
        "gtt_modify_order.json",
    ),
    (
        &["gtt", "delete", "--trigger-id", "123", "--confirm"],
        "DELETE",
        "/gtt/triggers/123",
        "gtt_delete_order.json",
    ),
    (
        &[
            "portfolio",
            "convert",
            "--exchange",
            "NSE",
            "--tradingsymbol",
            "INFY",
            "--transaction-type",
            "BUY",
            "--position-type",
            "day",
            "--quantity",
            "1",
            "--old-product",
            "MIS",
            "--new-product",
            "CNC",
            "--confirm",
        ],
        "PUT",
        "/portfolio/positions",
        "convert_position.json",
    ),
];

#[test]
fn remaining_write_fixtures_and_raw_requests_keep_json_bytes() {
    for &(args, method, endpoint, file) in WRITE_CASES {
        let setup = Setup::new();
        let body = fixture(file);
        setup.server.mock(|when, then| {
            when.method(method).path(endpoint);
            then.status(200).body(&body);
        });
        let wire = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), args, true);
        assert_eq!(wire.status.code(), Some(0), "{args:?}");
        assert_json_golden(
            &setup,
            &format!("json-{}-live", file.trim_end_matches(".json")),
            &wire,
        );
        let dry = setup.run(
            Path::new(env!("CARGO_BIN_EXE_kite")),
            &args[..args.len() - 1],
            true,
        );
        assert_json_golden(
            &setup,
            &format!("json-{}-dry", file.trim_end_matches(".json")),
            &dry,
        );
    }
    let setup = Setup::new();
    setup.server.mock(|when, then| {
        when.path("/raw");
        then.status(200)
            .header("content-type", "text/plain")
            .body("hello");
    });
    for args in [
        vec!["request", "get", "/raw"],
        vec![
            "request",
            "post",
            "/raw",
            "--form",
            "safe=value",
            "--confirm",
        ],
        vec!["request", "put", "/raw", "--body-json", "{}", "--confirm"],
        vec!["request", "delete", "/raw", "--confirm"],
    ] {
        let wire = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &args, true);
        assert_eq!(wire.status.code(), Some(0));
        assert_json_golden(&setup, &format!("json-raw-{}", args[1]), &wire);
    }
}

#[test]
fn cached_instrument_commands_keep_json_bytes_and_use_human_tables() {
    let setup = Setup::new();
    let cache_path = setup.cache_path();
    let directory = cache_path.parent().unwrap();
    fs::create_dir_all(directory).unwrap();
    let cache = directory.join("instruments.json");
    let records = json!([{"instrument_token":408_065,"tradingsymbol":"INFY","name":"Infosys","exchange":"NSE","expiry":null,"strike":null,"tick_size":0.05,"lot_size":1,"instrument_type":"EQ","segment":"NSE","freeze_quantity":null}]);
    fs::write(&cache, records.to_string()).unwrap();
    for args in [
        vec!["market", "search", "INFY"],
        vec!["market", "resolve", "NSE:INFY"],
        vec!["market", "instruments", "--limit", "1"],
        vec!["market", "cache", "path"],
    ] {
        let wire = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &args, true);
        assert_eq!(wire.status.code(), Some(0), "{args:?}");
        let name = if args[1] == "cache" {
            format!("json-market-cache-{}", args[2])
        } else {
            format!("json-market-{}", args[1])
        };
        assert_json_golden(&setup, &name, &wire);
        let human = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &args, false);
        assert_eq!(human.status.code(), Some(0));
        assert!(!human.stdout.starts_with(b"{"));
        if args == ["market", "cache", "path"] {
            assert_eq!(human.stdout, format!("{}\n", cache.display()).as_bytes());
            assert_eq!(human.stderr, b"");
        }
    }
    let out = setup.home.join("export.json");
    let args = ["market", "instruments", "--out", out.to_str().unwrap()];
    let wire = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &args, true);
    let exported = fs::read(&out).unwrap();
    assert_json_golden(&setup, "json-market-export", &wire);
    assert_eq!(exported, fs::read(out).unwrap());
    fs::OpenOptions::new()
        .write(true)
        .open(&cache)
        .unwrap()
        .set_times(
            fs::FileTimes::new().set_modified(
                std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000),
            ),
        )
        .unwrap();
    let args = ["market", "cache", "info"];
    let wire = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &args, true);
    assert_json_golden(&setup, "json-market-cache-info", &wire);
    let human = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &args, false);
    assert_eq!(human.status.code(), Some(0));
    assert!(!human.stdout.starts_with(b"{"));
    let args = ["market", "cache", "clear"];
    let wire = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &args, true);
    assert_json_golden(&setup, "json-market-cache-clear", &wire);
}

#[test]
fn missing_holding_fields_stay_unknown_and_do_not_change_the_calculation() {
    for pointer in [
        "/average_price",
        "/last_price",
        "/close_price",
        "/quantity",
        "/t1_quantity",
        "/collateral_quantity",
        "/mtf/quantity",
    ] {
        let setup = Setup::new();
        let mut body: Value = serde_json::from_str(&fixture("holdings.json")).unwrap();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        body["data"][0]
            .pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(key);
        setup.server.mock(|when, then| {
            when.method(GET).path("/portfolio/holdings");
            then.status(200).json_body(body);
        });
        let human = setup.run(
            Path::new(env!("CARGO_BIN_EXE_kite")),
            &["portfolio", "holdings"],
            false,
        );
        assert_eq!(human.status.code(), Some(0), "{pointer}");
        let text = String::from_utf8(human.stdout).unwrap();
        let first = text
            .lines()
            .nth(1)
            .unwrap()
            .split_whitespace()
            .collect::<Vec<_>>();
        assert_eq!(first[0], "NSE:AARON");
        assert_eq!(first[1], "–", "{pointer}");
        assert_eq!(&first[4..7], &["–", "–", "–"], "{pointer}");
        assert!(
            text.contains("Invested ₹12,828.50 · Current ₹12,199.20 · Day's P&L -₹63.20"),
            "{pointer}: {text}"
        );
        assert_eq!(
            human.stderr,
            b"warning: excluded 1 incomplete, discrepant or unpriced holdings from totals\n"
        );
        let wire = setup.run(
            Path::new(env!("CARGO_BIN_EXE_kite")),
            &["portfolio", "holdings"],
            true,
        );
        assert_json_golden(
            &setup,
            &format!("json-holdings-missing{}", pointer.replace('/', "-")),
            &wire,
        );
    }
}

#[test]
fn raw_json_dry_runs_keep_types_structure_and_redaction() {
    let setup = Setup::new();
    let args = [
        "request",
        "post",
        "/raw",
        "--body-json",
        r#"{"a":[1,2],"b":"1, 2","c":null,"d":true,"nested":{"access_token":"sentinel-secret","items":[{"n":3}]}}"#,
    ];
    let human = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &args, false);
    assert_eq!(human.status.code(), Some(0));
    let text = String::from_utf8(human.stdout).unwrap();
    assert_eq!(
        text,
        "DRY RUN  nothing was sent to Kite\n\nPOST /raw\nbody  {\"a\":[1,2],\"b\":\"1, 2\",\"c\":null,\"d\":true,\"nested\":{\"access_token\":\"<redacted>\",\"items\":[{\"n\":3}]}}\n"
    );
    assert!(!text.contains("sentinel-secret"));
    let wire = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &args, true);
    assert_json_golden(&setup, "json-raw-json-dry-run", &wire);
}

#[test]
fn raw_error_envelopes_fail_without_claiming_success() {
    let setup = Setup::new();
    setup.server.mock(|when, then| {
        when.method(POST).path("/raw");
        then.status(200).json_body(json!({"status":"error","error_type":"InputException","message":"business request rejected"}));
    });
    let args = ["request", "post", "/raw", "--body-json", "{}", "--confirm"];
    let human = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &args, false);
    assert_eq!(human.status.code(), Some(1));
    assert!(
        !String::from_utf8(human.stdout)
            .unwrap()
            .contains("Request submitted")
    );
    assert_eq!(
        human.stderr,
        b"error: business request rejected (InputException, HTTP 200)\n"
    );
    let wire = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &args, true);
    assert_json_golden(&setup, "json-raw-error", &wire);
}

#[test]
fn external_field_names_cannot_inject_terminal_controls() {
    let setup = Setup::new();
    setup.server.mock(|when, then| {
        when.method(GET).path("/raw");
        then.status(200).json_body(json!({"outer\u{1b}[31m":{"inner\u{9b}31m":"value\u{1b}[0m"}, "rows":[{"key\u{1b}[32m":1}]}));
    });
    let args = ["request", "get", "/raw"];
    let human = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &args, false);
    assert_eq!(human.status.code(), Some(0));
    let text = String::from_utf8(human.stdout).unwrap();
    assert!(!text.chars().any(|ch| ch.is_control() && ch != '\n'));
    assert!(text.contains("outer [31m.inner 31m"), "{text}");
    assert!(text.contains("rows[0].key [32m"), "{text}");
    let wire = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &args, true);
    assert_json_golden(&setup, "json-raw-field-names", &wire);

    let dry = setup.run(
        Path::new(env!("CARGO_BIN_EXE_kite")),
        &["request", "post", "/raw", "--form", "key\u{1b}[31m=value"],
        false,
    );
    assert!(!dry.stdout.contains(&0x1b));
    assert!(String::from_utf8(dry.stdout).unwrap().contains("key [31m"));
}

#[test]
fn empty_position_groups_only_write_empty_sentences() {
    let setup = Setup::new();
    let mock = setup.server.mock(|when, then| {
        when.method(GET).path("/portfolio/positions");
        then.status(200)
            .json_body(json!({"status":"success","data":{"net":[],"day":[]}}));
    });
    let args = ["portfolio", "positions"];
    let human = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &args, false);
    assert_eq!(human.status.code(), Some(0));
    assert_eq!(human.stdout, b"");
    assert_eq!(human.stderr, b"No net positions.\nNo day positions.\n");
    let wire = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &args, true);
    assert_json_golden(&setup, "json-empty-positions", &wire);
    mock.assert_calls(2);
}

#[test]
fn conversion_outputs_show_both_products_without_order_defaults() {
    let setup = Setup::new();
    setup.server.mock(|when, then| {
        when.method(PUT).path("/portfolio/positions");
        then.status(200).body(fixture("convert_position.json"));
    });
    let args = WRITE_CASES.last().unwrap().0;
    for args in [args, &args[..args.len() - 1]] {
        let human = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), args, false);
        assert_eq!(human.status.code(), Some(0));
        let text = String::from_utf8(human.stdout).unwrap();
        for field in [
            "old_product",
            "MIS",
            "new_product",
            "CNC",
            "position_type",
            "day",
            "INFY",
        ] {
            assert!(text.contains(field), "{text}");
        }
        assert!(!text.contains("DAY"));
        assert!(!text.contains("order_type"));
        assert!(!text.contains("validity"));
    }
}

#[test]
fn remaining_read_options_and_raw_dry_runs_match_the_baseline() {
    let setup = Setup::new();
    for &(endpoint, file) in &[
        ("/orders", "orders.json"),
        ("/quote", "quote.json"),
        ("/quote/ohlc", "ohlc.json"),
        ("/user/margins", "margins.json"),
        ("/margins/orders", "order_margins.json"),
        ("/margins/basket", "basket_margins.json"),
    ] {
        setup.server.mock(|when, then| {
            when.path(endpoint);
            then.status(200).body(fixture(file));
        });
    }
    setup.server.mock(|when, then| {
        when.path("/user/margins/commodity");
        let margins: Value = serde_json::from_str(&fixture("margins.json")).unwrap();
        then.status(200)
            .json_body(json!({"status":"success", "data":margins["data"]["commodity"]}));
    });
    for (index, args) in [
        vec!["orders", "list", "--status", "OPEN"],
        vec!["orders", "list", "--tradingsymbol", "IOC"],
        vec![
            "orders",
            "list",
            "--status",
            "COMPLETE",
            "--tradingsymbol",
            "IOC",
        ],
        vec!["quotes", "full", "NSE:INFY"],
        vec!["quotes", "ohlc", "NSE:INFY"],
        vec!["user", "margins", "--segment", "commodity"],
        vec!["margins", "orders", "--input-json", "[]", "--compact"],
        vec!["margins", "basket", "--input-json", "[]", "--compact"],
        vec![
            "margins",
            "basket",
            "--input-json",
            "[]",
            "--consider-positions",
        ],
        vec!["doctor"],
        vec!["config", "show", "--reveal"],
        vec![
            "request",
            "post",
            "/raw",
            "--form",
            "safe=value",
            "--dry-run",
        ],
        vec![
            "request",
            "put",
            "/raw",
            "--body-json",
            "[1,true,null]",
            "--dry-run",
        ],
        vec![
            "request",
            "delete",
            "/raw",
            "--query",
            "safe=value",
            "--dry-run",
        ],
    ]
    .into_iter()
    .enumerate()
    {
        let wire = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &args, true);
        assert_eq!(wire.status.code(), Some(0), "{args:?}");
        assert_json_golden(&setup, &format!("json-read-options-{index:02}"), &wire);
    }
}

const ALERT_UUID: &str = "550e8400-e29b-41d4-a716-446655440000";
const ALERT_CREATE: &[&str] = &[
    "alerts",
    "create",
    "--name",
    "NIFTY 50",
    "--lhs",
    "NSE:NIFTY 50",
    "--operator",
    ">=",
    "--value",
    "27000",
];

fn cache_alert_instruments(setup: &Setup) {
    let cache_path = setup.cache_path();
    let directory = cache_path.parent().unwrap();
    fs::create_dir_all(directory).unwrap();
    let records: Vec<_> = [(256_265, "NIFTY 50", "INDICES"), (408_065, "INFY", "NSE")]
        .into_iter()
        .map(|(token, symbol, segment)| {
            json!({
                "instrument_token":token,"tradingsymbol":symbol,"name":symbol,"exchange":"NSE",
                "tick_size":0.05,"lot_size":1,"instrument_type":"EQ","segment":segment
            })
        })
        .collect();
    fs::write(
        directory.join("instruments.json"),
        serde_json::to_vec(&records).unwrap(),
    )
    .unwrap();
}

#[test]
fn alerts_fixture_reads_preserve_json_and_render_tables() {
    for (name, args, endpoint, file) in [
        (
            "alerts-list",
            vec!["alerts", "list"],
            "/alerts".to_string(),
            "alerts_get.json",
        ),
        (
            "alerts-get",
            vec!["alerts", "get", ALERT_UUID],
            format!("/alerts/{ALERT_UUID}"),
            "alerts_get_one.json",
        ),
        (
            "alerts-history",
            vec!["alerts", "history", ALERT_UUID],
            format!("/alerts/{ALERT_UUID}/history"),
            "alerts_history.json",
        ),
    ] {
        let setup = Setup::new();
        let body = fixture(file);
        let mock = setup.server.mock(|when, then| {
            when.method(GET).path(endpoint);
            then.status(200).body(&body);
        });
        let human = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &args, false);
        assert_eq!(
            human.status.code(),
            Some(0),
            "{}",
            setup.normalized(&human.stderr)
        );
        assert_golden(&setup, name, &human);
        let wire = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &args, true);
        let expected: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            wire.stdout,
            format!(
                "{}\n",
                serde_json::to_string_pretty(&expected["data"]).unwrap()
            )
            .as_bytes()
        );
        assert_json_golden(&setup, &format!("json-{name}"), &wire);
        mock.assert_calls(2);
    }
}

#[test]
fn alerts_status_filter_is_sent_to_the_server() {
    let setup = Setup::new();
    let mock = setup.server.mock(|when, then| {
        when.method(GET)
            .path("/alerts")
            .query_param("status", "enabled")
            .query_param_count(".*", ".*", 1);
        then.status(200).body(fixture("alerts_get.json"));
    });
    let output = setup.run(
        Path::new(env!("CARGO_BIN_EXE_kite")),
        &["alerts", "list", "--status", "enabled"],
        true,
    );
    assert_eq!(output.status.code(), Some(0));
    mock.assert_calls(1);
}

#[test]
fn alerts_writes_show_exact_dry_runs_and_send_exact_forms_only_when_confirmed() {
    for (name, args, method, endpoint, form, response) in [
        (
            "alerts-create",
            ALERT_CREATE.to_vec(),
            POST,
            "/alerts".to_string(),
            json!({"name":"NIFTY 50","type":"simple","lhs_exchange":"INDICES","lhs_tradingsymbol":"NIFTY 50","lhs_attribute":"LastTradedPrice","operator":">=","rhs_type":"constant","rhs_constant":"27000"}),
            "alerts_create.json",
        ),
        (
            "alerts-modify",
            vec!["alerts", "modify", ALERT_UUID, "--value", "28000"],
            PUT,
            format!("/alerts/{ALERT_UUID}"),
            json!({"name":"NIFTY 50","type":"simple","lhs_exchange":"INDICES","lhs_tradingsymbol":"NIFTY 50","lhs_attribute":"LastTradedPrice","operator":">=","rhs_type":"constant","rhs_constant":"28000","status":"enabled"}),
            "alerts_modify.json",
        ),
        (
            "alerts-delete",
            vec!["alerts", "delete", ALERT_UUID, "second-uuid"],
            DELETE,
            "/alerts".to_string(),
            json!({}),
            "alerts_delete.json",
        ),
        (
            "alerts-enable",
            vec!["alerts", "enable", ALERT_UUID],
            PUT,
            format!("/alerts/{ALERT_UUID}"),
            json!({"status":"enabled","type":"simple"}),
            "alerts_modify.json",
        ),
        (
            "alerts-disable",
            vec!["alerts", "disable", ALERT_UUID],
            PUT,
            format!("/alerts/{ALERT_UUID}"),
            json!({"status":"disabled","type":"simple"}),
            "alerts_modify.json",
        ),
    ] {
        let setup = Setup::new();
        cache_alert_instruments(&setup);
        setup.server.mock(|when, then| {
            when.method(GET).path(format!("/alerts/{ALERT_UUID}"));
            then.status(200).body(fixture("alerts_get_one.json"));
        });
        let write = setup.server.mock(|mut when, then| {
            when = when.method(method).path(&endpoint);
            if name == "alerts-delete" {
                when.query_param("uuid", ALERT_UUID)
                    .query_param("uuid", "second-uuid")
                    .query_param_count(".*", ".*", 2)
                    .body("");
            } else {
                let fields = form.as_object().unwrap();
                when = when.form_urlencoded_tuple_count(".*", ".*", fields.len());
                for (key, value) in fields {
                    when = when.form_urlencoded_tuple(key, value.as_str().unwrap());
                }
            }
            then.status(200).body(fixture(response));
        });
        for json in [false, true] {
            let dry = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &args, json);
            assert_eq!(
                dry.status.code(),
                Some(0),
                "{name}: {}",
                setup.normalized(&dry.stderr)
            );
            if json {
                let value: Value = serde_json::from_slice(&dry.stdout).unwrap();
                assert_eq!(
                    value["body"]
                        .get("form")
                        .cloned()
                        .unwrap_or_else(|| json!({})),
                    form
                );
                assert_eq!(value["dry_run"], true);
                assert_eq!(value["path"], endpoint);
                assert_json_golden(&setup, &format!("json-{name}-dry-run"), &dry);
            } else {
                assert_golden(&setup, &format!("{name}-dry-run"), &dry);
            }
            write.assert_calls(0);
        }
        let confirmed = [args, vec!["--confirm"]].concat();
        let result = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &confirmed, false);
        assert_eq!(
            result.status.code(),
            Some(0),
            "{name}: {}",
            setup.normalized(&result.stderr)
        );
        assert_golden(&setup, &format!("{name}-submitted"), &result);
        write.assert_calls(1);
    }
}

#[test]
fn alerts_instrument_conditions_resolve_both_sides_and_attributes() {
    let setup = Setup::new();
    cache_alert_instruments(&setup);
    let args = [
        "alerts",
        "create",
        "--name",
        "Compare",
        "--lhs",
        "nse:infy",
        "--attribute",
        "HighPrice",
        "--operator",
        ">",
        "--rhs",
        "NSE:NIFTY 50",
        "--rhs-attribute",
        "LowPrice",
    ];
    let expected = json!({"name":"Compare","type":"simple","lhs_exchange":"NSE","lhs_tradingsymbol":"INFY","lhs_attribute":"HighPrice","operator":">","rhs_type":"instrument","rhs_exchange":"INDICES","rhs_tradingsymbol":"NIFTY 50","rhs_attribute":"LowPrice"});
    let mock = setup.server.mock(|mut when, then| {
        when = when
            .method(POST)
            .path("/alerts")
            .form_urlencoded_tuple_count(".*", ".*", 10);
        for (key, value) in expected.as_object().unwrap() {
            when = when.form_urlencoded_tuple(key, value.as_str().unwrap());
        }
        then.status(200).body(fixture("alerts_create.json"));
    });
    let dry = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &args, true);
    assert_eq!(
        dry.status.code(),
        Some(0),
        "{}",
        setup.normalized(&dry.stderr)
    );
    let value: Value = serde_json::from_slice(&dry.stdout).unwrap();
    assert_eq!(value["body"]["form"], expected);
    mock.assert_calls(0);
    let mut confirmed = args.to_vec();
    confirmed.push("--confirm");
    let result = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &confirmed, false);
    assert_eq!(
        result.status.code(),
        Some(0),
        "{}",
        setup.normalized(&result.stderr)
    );
    mock.assert_calls(1);
}

#[test]
fn alerts_modify_preserves_disabled_instrument_conditions_and_allows_rhs_changes() {
    let setup = Setup::new();
    cache_alert_instruments(&setup);
    let mut body: Value = serde_json::from_str(&fixture("alerts_get_one.json")).unwrap();
    body["data"]["status"] = json!("disabled");
    body["data"]["rhs_type"] = json!("instrument");
    body["data"]["rhs_exchange"] = json!("NSE");
    body["data"]["rhs_tradingsymbol"] = json!("INFY");
    body["data"]["rhs_attribute"] = json!("HighPrice");
    setup.server.mock(|when, then| {
        when.method(GET).path(format!("/alerts/{ALERT_UUID}"));
        then.status(200).json_body(body);
    });
    for (flags, expected) in [
        (
            vec!["--name", "Renamed"],
            json!({"name":"Renamed","type":"simple","status":"disabled","lhs_exchange":"INDICES","lhs_tradingsymbol":"NIFTY 50","lhs_attribute":"LastTradedPrice","operator":">=","rhs_type":"instrument","rhs_exchange":"NSE","rhs_tradingsymbol":"INFY","rhs_attribute":"HighPrice"}),
        ),
        (
            vec![
                "--lhs",
                "NSE:INFY",
                "--attribute",
                "LowPrice",
                "--operator",
                "<=",
                "--rhs-attribute",
                "ClosePrice",
            ],
            json!({"name":"NIFTY 50","type":"simple","status":"disabled","lhs_exchange":"NSE","lhs_tradingsymbol":"INFY","lhs_attribute":"LowPrice","operator":"<=","rhs_type":"instrument","rhs_exchange":"NSE","rhs_tradingsymbol":"INFY","rhs_attribute":"ClosePrice"}),
        ),
        (
            vec!["--rhs", "NSE:NIFTY 50"],
            json!({"name":"NIFTY 50","type":"simple","status":"disabled","lhs_exchange":"INDICES","lhs_tradingsymbol":"NIFTY 50","lhs_attribute":"LastTradedPrice","operator":">=","rhs_type":"instrument","rhs_exchange":"INDICES","rhs_tradingsymbol":"NIFTY 50","rhs_attribute":"HighPrice"}),
        ),
        (
            vec!["--value", "-1.5"],
            json!({"name":"NIFTY 50","type":"simple","status":"disabled","lhs_exchange":"INDICES","lhs_tradingsymbol":"NIFTY 50","lhs_attribute":"LastTradedPrice","operator":">=","rhs_type":"constant","rhs_constant":"-1.5"}),
        ),
    ] {
        let mut args = vec!["alerts", "modify", ALERT_UUID];
        args.extend(flags);
        let dry = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &args, true);
        assert_eq!(
            dry.status.code(),
            Some(0),
            "{}",
            setup.normalized(&dry.stderr)
        );
        let value: Value = serde_json::from_slice(&dry.stdout).unwrap();
        assert_eq!(value["body"]["form"], expected);
    }
}

#[test]
fn alerts_status_changes_preserve_ato_type_and_reject_ato_edits() {
    let setup = Setup::new();
    let body: Value = serde_json::from_str(&fixture("alerts_get.json")).unwrap();
    setup.server.mock(|when, then| {
        when.method(GET).path(format!("/alerts/{ALERT_UUID}"));
        then.status(200)
            .json_body(json!({"status":"success","data":body["data"][1]}));
    });
    for (command, status) in [("enable", "enabled"), ("disable", "disabled")] {
        let mock = setup.server.mock(|when, then| {
            when.method(PUT)
                .path(format!("/alerts/{ALERT_UUID}"))
                .form_urlencoded_tuple("type", "ato")
                .form_urlencoded_tuple("status", status)
                .form_urlencoded_tuple_count(".*", ".*", 2);
            then.status(200).body(fixture("alerts_modify.json"));
        });
        let dry = setup.run(
            Path::new(env!("CARGO_BIN_EXE_kite")),
            &["alerts", command, ALERT_UUID],
            true,
        );
        assert_eq!(dry.status.code(), Some(0));
        let value: Value = serde_json::from_slice(&dry.stdout).unwrap();
        assert_eq!(value["body"]["form"], json!({"type":"ato","status":status}));
        mock.assert_calls(0);
        let live = setup.run(
            Path::new(env!("CARGO_BIN_EXE_kite")),
            &["alerts", command, ALERT_UUID, "--confirm"],
            false,
        );
        assert_eq!(live.status.code(), Some(0));
        mock.assert_calls(1);
    }
    let invalid = setup.run(
        Path::new(env!("CARGO_BIN_EXE_kite")),
        &["alerts", "modify", ALERT_UUID, "--value", "1", "--confirm"],
        false,
    );
    assert_eq!(invalid.status.code(), Some(1));
    assert_eq!(invalid.stdout, b"");
    assert_eq!(
        invalid.stderr,
        b"error: use the TUI to edit ATO basket alerts\n"
    );
}

#[test]
fn alerts_invalid_arguments_exit_two_without_requests() {
    let setup = Setup::new();
    let writes = setup.server.mock(|when, then| {
        when.path("/alerts");
        then.status(500);
    });
    for (args, message) in [
        (
            vec![
                "alerts",
                "create",
                "--name",
                "Test",
                "--lhs",
                "NSE:INFY",
                "--operator",
                "!=",
                "--value",
                "1",
            ],
            "invalid value '!='",
        ),
        (
            vec![
                "alerts",
                "create",
                "--name",
                "Test",
                "--lhs",
                "NSE:INFY",
                "--operator",
                ">=",
            ],
            "--value <VALUE>|--rhs <EXCHANGE:SYMBOL>",
        ),
        (
            vec![
                "alerts",
                "create",
                "--name",
                "Test",
                "--lhs",
                "NSE:INFY",
                "--operator",
                ">=",
                "--value",
                "NaN",
            ],
            "alert value must be a finite number",
        ),
        (
            vec![
                "alerts",
                "create",
                "--name",
                "Test",
                "--lhs",
                "NSE:INFY",
                "--operator",
                ">=",
                "--value",
                "1",
                "--rhs",
                "NSE:INFY",
            ],
            "cannot be used with",
        ),
        (vec!["alerts", "delete"], "required arguments"),
        (
            vec!["alerts", "list", "--status", "unknown"],
            "invalid value 'unknown'",
        ),
    ] {
        let invalid = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &args, false);
        assert_eq!(invalid.status.code(), Some(2), "{args:?}");
        assert_eq!(invalid.stdout, b"");
        assert!(
            setup.normalized(&invalid.stderr).contains(message),
            "{}",
            setup.normalized(&invalid.stderr)
        );
    }
    writes.assert_calls(0);
}

#[test]
fn alerts_empty_results_and_session_errors_use_existing_diagnostics() {
    let setup = Setup::new();
    let mut empty = setup.server.mock(|when, then| {
        when.method(GET).path("/alerts");
        then.status(200)
            .json_body(json!({"status":"success","data":[]}));
    });
    let result = setup.run(
        Path::new(env!("CARGO_BIN_EXE_kite")),
        &["alerts", "list"],
        false,
    );
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(result.stdout, b"");
    assert_eq!(result.stderr, b"No alerts.\n");
    empty.delete();
    setup.server.mock(|when, then| {
        when.method(GET).path("/alerts");
        then.status(403).json_body(
            json!({"status":"error","message":"Invalid session","error_type":"TokenException"}),
        );
    });
    let result = setup.run(
        Path::new(env!("CARGO_BIN_EXE_kite")),
        &["alerts", "list"],
        false,
    );
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(result.stdout, b"");
    assert_eq!(
        result.stderr,
        b"error: Invalid session (TokenException, HTTP 403)\nhint: run `kite auth login` to sign in again\n"
    );
}

#[test]
fn alerts_explicit_dry_run_overrides_confirmation() {
    let setup = Setup::new();
    cache_alert_instruments(&setup);
    let write = setup.server.mock(|when, then| {
        when.method(POST).path("/alerts");
        then.status(500);
    });
    let args = [ALERT_CREATE, &["--confirm", "--dry-run"]].concat();
    let dry = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &args, false);
    assert_eq!(dry.status.code(), Some(0));
    assert_golden(&setup, "alerts-create-dry-run", &dry);
    write.assert_calls(0);
}

#[test]
fn rejected_raw_writes_exit_one_in_human_and_json_modes() {
    let setup = Setup::new();
    let mock = setup.server.mock(|when, then| {
        when.path("/orders/regular");
        then.status(200).json_body(json!({
            "status":"error", "error_type":"OrderException", "message":"Insufficient funds"
        }));
    });
    for method in ["post", "put", "delete"] {
        for json_mode in [false, true] {
            let result = setup.run(
                Path::new(env!("CARGO_BIN_EXE_kite")),
                &["request", method, "/orders/regular", "--confirm"],
                json_mode,
            );
            assert_eq!(result.status.code(), Some(1));
            if json_mode {
                let body: Value = serde_json::from_slice(&result.stdout).unwrap();
                assert_eq!(body["ok"], false);
                assert!(
                    body["error"]["message"]
                        .as_str()
                        .unwrap()
                        .contains("Insufficient funds")
                );
            } else {
                assert!(
                    String::from_utf8(result.stderr)
                        .unwrap()
                        .contains("Insufficient funds")
                );
            }
        }
    }
    mock.assert_calls(6);
}

#[test]
fn env_only_profile_reads_do_not_create_config() {
    let setup = Setup::new();
    fs::remove_file(&setup.config).unwrap();
    let mock = setup.server.mock(|when, then| {
        when.method(GET).path("/user/profile");
        then.status(200).body(fixture("profile.json"));
    });
    let mut command = Command::new(env!("CARGO_BIN_EXE_kite"));
    setup.configure_command(&mut command);
    let result = command
        .env("HOME", &setup.home)
        .env("XDG_CONFIG_HOME", &setup.home)
        .env("XDG_CACHE_HOME", &setup.home)
        .env("KITE_API_ROOT", setup.server.base_url())
        .env("KITE_API_KEY", "test-key")
        .env("KITE_ACCESS_TOKEN", "test-token")
        .args([
            "--config",
            setup.config.to_str().unwrap(),
            "--json",
            "user",
            "profile",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    assert!(!setup.config.exists());
    mock.assert_calls(1);
}

#[test]
fn gtt_modify_preserves_stored_type_in_dry_runs_and_live_writes() {
    for trigger_type in ["two-leg", "trailing-two-leg"] {
        let setup = Setup::new();
        let read = setup.server.mock(|when, then| {
            when.method(GET).path("/gtt/triggers/123");
            then.status(200)
                .json_body(json!({"status":"success","data":{"type":trigger_type}}));
        });
        let write = setup.server.mock(|when, then| {
            when.method(PUT)
                .path("/gtt/triggers/123")
                .form_urlencoded_tuple("type", trigger_type);
            then.status(200).body(fixture("gtt_modify_order.json"));
        });
        let condition = if trigger_type.starts_with("trailing-") {
            "{\"trailing_points\":[5]}"
        } else {
            "{}"
        };
        let args = [
            "gtt",
            "modify",
            "--trigger-id",
            "123",
            "--condition-json",
            condition,
            "--orders-json",
            "[]",
        ];
        let dry = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &args, true);
        assert_eq!(
            dry.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&dry.stdout)
        );
        let body: Value = serde_json::from_slice(&dry.stdout).unwrap();
        assert_eq!(body["body"]["form"]["type"], trigger_type);
        write.assert_calls(0);
        let mut confirmed = args.to_vec();
        confirmed.push("--confirm");
        let live = setup.run(Path::new(env!("CARGO_BIN_EXE_kite")), &confirmed, true);
        assert_eq!(
            live.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&live.stdout)
        );
        read.assert_calls(2);
        write.assert_calls(1);
    }
}

#[test]
fn json_output_to_a_closed_pipe_exits_successfully() {
    let setup = Setup::new();
    setup.server.mock(|when, then| {
        when.method(GET).path("/user/profile");
        then.status(200)
            .json_body(json!({"status":"success", "data":{"padding":"x".repeat(1_000_000)}}));
    });
    let mut command = Command::new(env!("CARGO_BIN_EXE_kite"));
    setup.configure_command(&mut command);
    let mut child = command
        .env("HOME", &setup.home)
        .args([
            "--config",
            setup.config.to_str().unwrap(),
            "--json",
            "user",
            "profile",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let result = child.wait_with_output().unwrap();
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(result.stderr, b"");
}

#[test]
fn a_dropped_write_reports_unknown_outcome_without_retrying() {
    use std::io::Read;
    for json_mode in [false, true] {
        let setup = Setup::new();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            loop {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream.set_nonblocking(false).unwrap();
                        stream
                            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                            .unwrap();
                        let mut bytes = [0; 8192];
                        let count = stream.read(&mut bytes).unwrap();
                        return Ok(String::from_utf8_lossy(&bytes[..count]).into_owned());
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        if std::time::Instant::now() >= deadline {
                            return Err("timed out waiting for the CLI request".to_owned());
                        }
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    Err(error) => return Err(format!("accepting the CLI request: {error}")),
                }
            }
        });
        let mut config: Value = serde_json::from_slice(&fs::read(&setup.config).unwrap()).unwrap();
        config["api_root"] = json!(format!("http://{address}"));
        fs::write(&setup.config, config.to_string()).unwrap();
        let result = setup.run(
            Path::new(env!("CARGO_BIN_EXE_kite")),
            &["request", "post", "/orders/regular", "--confirm"],
            json_mode,
        );
        assert_eq!(
            result.status.code(),
            Some(1),
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        let text = if json_mode {
            String::from_utf8_lossy(&result.stdout).into_owned()
        } else {
            String::from_utf8_lossy(&result.stderr).into_owned()
        };
        assert!(text.contains("write outcome is unknown"), "{text}");
        assert!(text.contains("before retrying"), "{text}");
        let request = server.join().unwrap().unwrap_or_else(|error| {
            panic!(
                "{error}; CLI stdout: {}; stderr: {}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            )
        });
        assert!(request.starts_with("POST /orders/regular "), "{request}");
    }
}

#[test]
fn alerts_modify_without_changes_is_a_usage_error() {
    let setup = Setup::new();
    let result = setup.run(
        Path::new(env!("CARGO_BIN_EXE_kite")),
        &["alerts", "modify", ALERT_UUID],
        false,
    );
    assert_eq!(result.status.code(), Some(2));
    assert!(
        String::from_utf8(result.stderr)
            .unwrap()
            .contains("required arguments")
    );
}

#[test]
fn zero_limit_returns_all_instrument_matches() {
    let setup = Setup::new();
    cache_alert_instruments(&setup);
    for command in ["search", "resolve"] {
        let result = setup.run(
            Path::new(env!("CARGO_BIN_EXE_kite")),
            &["market", command, "INFY", "--limit", "0"],
            true,
        );
        assert_eq!(result.status.code(), Some(0));
        let body: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_ne!(body["matches"].as_array().unwrap().len(), 0);
    }
}

#[test]
fn open_order_filter_includes_working_statuses() {
    let setup = Setup::new();
    let statuses = [
        "OPEN",
        "TRIGGER PENDING",
        "MODIFY PENDING",
        "AMO REQ RECEIVED",
        "COMPLETE",
        "CANCELLED",
    ];
    setup.server.mock(|when, then| {
        when.method(GET).path("/orders");
        then.status(200).json_body(
            json!({"status":"success","data":statuses.map(|status| json!({"status":status}))}),
        );
    });
    let result = setup.run(
        Path::new(env!("CARGO_BIN_EXE_kite")),
        &["orders", "list", "--status", "open"],
        true,
    );
    assert_eq!(result.status.code(), Some(0));
    let body: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(body.as_array().unwrap().len(), 4);
    assert!(
        body.as_array()
            .unwrap()
            .iter()
            .all(|row| row["status"] != "COMPLETE" && row["status"] != "CANCELLED")
    );
}

#[test]
fn mixed_case_order_flags_send_canonical_values() {
    let setup = Setup::new();
    let result = setup.run(
        Path::new(env!("CARGO_BIN_EXE_kite")),
        &[
            "orders",
            "place",
            "--variety",
            "REGULAR",
            "--exchange",
            "nse",
            "--tradingsymbol",
            "INFY",
            "--product",
            "cnc",
            "--transaction-type",
            "buy",
            "--order-type",
            "limit",
            "--quantity",
            "1",
        ],
        true,
    );
    assert_eq!(result.status.code(), Some(0));
    let body: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(body["path"], "/orders/regular");
    assert_eq!(body["body"]["form"]["exchange"], "NSE");
    assert_eq!(body["body"]["form"]["product"], "CNC");
    assert_eq!(body["body"]["form"]["transaction_type"], "BUY");
    assert_eq!(body["body"]["form"]["order_type"], "LIMIT");
}

#[test]
fn order_summary_preserves_price_trigger_and_omitted_validity() {
    let setup = Setup::new();
    let result = setup.run(
        Path::new(env!("CARGO_BIN_EXE_kite")),
        &[
            "orders",
            "place",
            "--exchange",
            "NSE",
            "--tradingsymbol",
            "INFY",
            "--product",
            "CNC",
            "--transaction-type",
            "BUY",
            "--order-type",
            "SL",
            "--quantity",
            "1",
            "--price",
            "1500.123456789",
            "--trigger-price",
            "1490.987654321",
        ],
        false,
    );
    assert_eq!(result.status.code(), Some(0));
    let text = String::from_utf8(result.stdout).unwrap();
    assert!(
        text.contains("SL @ 1500.123456789 trigger 1490.987654321  CNC"),
        "{text}"
    );
    assert!(!text.contains("DAY"));
}

#[test]
fn mutual_fund_human_pnl_is_computed_and_json_stays_raw() {
    let setup = Setup::new();
    setup.server.mock(|when, then| {
        when.method(GET).path("/mf/holdings");
        then.status(200).json_body(json!({"status":"success","data":[{"fund":"Example","folio":"test","quantity":10,"average_price":80,"last_price":100,"pnl":0}]}));
    });
    let human = setup.run(
        Path::new(env!("CARGO_BIN_EXE_kite")),
        &["mf", "holdings"],
        false,
    );
    assert_eq!(human.status.code(), Some(0));
    assert!(String::from_utf8(human.stdout).unwrap().contains("200.00"));
    let raw = setup.run(
        Path::new(env!("CARGO_BIN_EXE_kite")),
        &["mf", "holdings"],
        true,
    );
    assert_eq!(raw.status.code(), Some(0));
    let body: Value = serde_json::from_slice(&raw.stdout).unwrap();
    assert_eq!(body[0]["pnl"], 0);
}

#[test]
fn clearing_an_absent_cache_reports_no_deletion() {
    let setup = Setup::new();
    let result = setup.run(
        Path::new(env!("CARGO_BIN_EXE_kite")),
        &["market", "cache", "clear"],
        true,
    );
    assert_eq!(result.status.code(), Some(0));
    let value: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value["cleared"], false);
    assert!(value["path"].as_str().is_some());
}

#[test]
fn resolving_no_instrument_fails_with_an_empty_match_list() {
    let setup = Setup::new();
    cache_alert_instruments(&setup);
    let result = setup.run(
        Path::new(env!("CARGO_BIN_EXE_kite")),
        &["market", "resolve", "NSE:ABSENT"],
        true,
    );
    assert_eq!(result.status.code(), Some(1));
    let value: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value["matches"], json!([]));
}

#[test]
fn invalid_order_and_market_arguments_fail_before_a_request() {
    let setup = Setup::new();
    let binary = Path::new(env!("CARGO_BIN_EXE_kite"));
    for extra in [
        vec!["--price", "NaN"],
        vec!["--trigger-price", "inf"],
        vec!["--iceberg-legs", "1"],
        vec!["--iceberg-legs", "51"],
        vec!["--tag", "has space"],
    ] {
        let mut args = PLACE.to_vec();
        if extra[0] == "--price" {
            let index = args.iter().position(|value| *value == "--price").unwrap();
            args.drain(index..index + 2);
        }
        args.extend(extra);
        let result = setup.run(binary, &args, true);
        assert_eq!(result.status.code(), Some(2), "{args:?}");
    }
    for args in [
        &["market", "ltp"][..],
        &["market", "ltp", "INFY"][..],
        &["stream", "ticks"][..],
    ] {
        assert_eq!(setup.run(binary, args, true).status.code(), Some(2));
    }
}
