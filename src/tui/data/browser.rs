use super::{DataEvent, api_err, send};
use crate::tui::event::AppEvent;
use kiteconnect::KiteConnect;
use kiteconnect::Request;
use serde_json::Value;
use std::process::Stdio;
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

const CDSL_AUTHORISATION_BASE: &str =
    "https://kite.zerodha.com/connect/portfolio/authorise/holdings";

pub fn holding_authorisation_url(response: &Value, api_key: &str) -> Result<String, String> {
    let request_id = response
        .get("request_id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| "CDSL authorisation response is missing request_id".to_string())?;
    let mut url = url::Url::parse(CDSL_AUTHORISATION_BASE).expect("valid authorisation URL");
    url.path_segments_mut()
        .expect("authorisation URL has a host")
        .push(api_key)
        .push(request_id);
    Ok(url.to_string())
}

pub fn authorisation_opener(os: &str, url: &str) -> Result<(String, Vec<String>), String> {
    match os {
        "linux" => Ok(("xdg-open".to_string(), vec![url.to_string()])),
        "macos" => Ok(("open".to_string(), vec![url.to_string()])),
        "windows" => Ok((
            "cmd".to_string(),
            vec![
                "/C".to_string(),
                "start".to_string(),
                String::new(),
                url.to_string(),
            ],
        )),
        other => Err(format!("unsupported platform for browser launch: {other}")),
    }
}

pub fn launch_external_link(url: &str) -> Result<Child, String> {
    let (program, args) = authorisation_opener(std::env::consts::OS, url)?;
    Command::new(&program)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| format!("could not start {program}: {err}"))
}

async fn report_browser_exit(tx: &mpsc::Sender<AppEvent>, url: &str, mut child: Child) {
    let result = child.wait().await;
    let error = match result {
        Ok(status) if status.success() => return,
        Ok(status) => format!("browser launcher exited with {status}"),
        Err(error) => format!("could not wait for the browser launcher: {error}"),
    };
    send(
        tx,
        DataEvent::ExternalLink(Err(format!("{error}. Open this URL manually: {url}"))),
    )
    .await;
}

pub fn spawn_external_link(tx: mpsc::Sender<AppEvent>, url: String) {
    tokio::spawn(async move {
        match launch_external_link(&url) {
            Ok(child) => {
                send(&tx, DataEvent::ExternalLink(Ok(url.clone()))).await;
                report_browser_exit(&tx, &url, child).await;
            }
            Err(error) => {
                send(
                    &tx,
                    DataEvent::ExternalLink(Err(format!("{error}. Open this URL manually: {url}"))),
                )
                .await;
            }
        }
    });
}

pub fn spawn_authorise_holdings(client: KiteConnect, tx: mpsc::Sender<AppEvent>) {
    tokio::spawn(async move {
        let handoff = async {
            let response = client
                .call(Request::post("/portfolio/holdings/authorise").form(vec![
                    ("overnight".to_string(), "true".to_string()),
                    ("transfer_type".to_string(), String::new()),
                ]))
                .raw()
                .await
                .map_err(api_err)?;
            let url = holding_authorisation_url(&response, client.api_key())?;
            let child = launch_external_link(&url)
                .map_err(|error| format!("{error}. Open this URL manually: {url}"))?;
            Ok::<_, String>((url, child))
        }
        .await;
        match handoff {
            Ok((url, child)) => {
                send(&tx, DataEvent::HoldingsAuthorised(Ok(url.clone()))).await;
                report_browser_exit(&tx, &url, child).await;
            }
            Err(error) => {
                send(&tx, DataEvent::HoldingsAuthorised(Err(error))).await;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{authorisation_opener, holding_authorisation_url};

    #[test]
    fn cdsl_authorisation_url_uses_the_api_key_and_request_id() {
        let url = holding_authorisation_url(
            &serde_json::json!({
                "request_id": "request-42"
            }),
            "test-key",
        )
        .expect("request id produces handoff URL");
        assert_eq!(
            url,
            "https://kite.zerodha.com/connect/portfolio/authorise/holdings/test-key/request-42"
        );
        assert!(holding_authorisation_url(&serde_json::json!({}), "test-key").is_err());
    }

    #[test]
    fn cdsl_authorisation_openers_are_platform_portable() {
        let url = "https://kite.zerodha.com/connect/portfolio/authorise/holdings/test-key/id";
        assert_eq!(
            authorisation_opener("linux", url),
            Ok(("xdg-open".to_string(), vec![url.to_string()]))
        );
        assert_eq!(
            authorisation_opener("macos", url),
            Ok(("open".to_string(), vec![url.to_string()]))
        );
        assert_eq!(
            authorisation_opener("windows", url),
            Ok((
                "cmd".to_string(),
                vec![
                    "/C".to_string(),
                    "start".to_string(),
                    String::new(),
                    url.to_string(),
                ],
            ))
        );
    }
    #[cfg(unix)]
    #[test]
    fn browser_launcher_is_asynchronous_and_reports_a_later_failure() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("kite-browser-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let launcher_name = authorisation_opener(std::env::consts::OS, "https://example.com")
            .expect("current platform has a browser opener")
            .0;
        let launcher = root.join(launcher_name);
        std::fs::write(
            &launcher,
            "#!/bin/sh\nwhile [ -d \"$HOME\" ] && [ ! -e \"$HOME/release\" ]; do /bin/sleep 0.05; done\nif [ -e \"$HOME/release\" ]; then exit 1; fi\nexit 0\n",
        )
        .unwrap();
        std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o700)).unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .env_clear()
            .env("PATH", &root)
            .env("HOME", &root)
            .env("XDG_CONFIG_HOME", &root)
            .env("XDG_CACHE_HOME", &root)
            .env("XDG_DATA_HOME", &root)
            .env("XDG_STATE_HOME", &root)
            .env("CARGO_HOME", &root)
            .env("RUSTUP_HOME", &root)
            .env("MISE_DATA_DIR", &root)
            .env("KITE_BROWSER_FIXTURE", "1")
            .args([
                "--exact",
                "tui::data::browser::tests::browser_launcher_child",
                "--nocapture",
            ])
            .output()
            .unwrap();
        std::fs::remove_dir_all(&root).unwrap();
        assert!(
            output.status.success(),
            "child stdout:\n{}\nchild stderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn browser_launcher_child() {
        use crate::tui::data::{DataEvent, tests::connect_client};
        use crate::tui::event::AppEvent;
        if std::env::var_os("KITE_BROWSER_FIXTURE").is_none() {
            return;
        }
        let server = httpmock::MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(httpmock::Method::POST)
                    .path("/portfolio/holdings/authorise");
                then.status(200).json_body(
                    serde_json::json!({"status":"success","data":{"request_id":"test-request"}}),
                );
            })
            .await;
        let (tx, mut rx) = tokio::sync::mpsc::channel(8);
        super::spawn_authorise_holdings(connect_client(&server), tx);
        let event = tokio::time::timeout(std::time::Duration::from_secs(10), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            event,
            AppEvent::Data(DataEvent::HoldingsAuthorised(Ok(_)))
        ));
        let home = std::env::var_os("HOME").expect("isolated HOME is set");
        std::fs::write(std::path::Path::new(&home).join("release"), "").unwrap();
        let event = tokio::time::timeout(std::time::Duration::from_secs(10), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(
            matches!(event, AppEvent::Data(DataEvent::ExternalLink(Err(message))) if message.contains("exited") && message.contains("Open this URL manually"))
        );
    }
}
