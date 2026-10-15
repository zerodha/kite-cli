use crate::app::output::Output;
use crate::app::render;
use crate::app::runtime::Runtime;
use crate::cli::{RawWriteArgs, RequestCommands};
use crate::util::{dry_run_request, parse_key_values, raw_body};
use anyhow::Result;
use kiteconnect::error::{ApiError, ErrorKind};
use kiteconnect::{KiteConnect, Request};
use reqwest::Method;
use serde_json::{Value, json};

pub async fn handle(rt: &Runtime, command: RequestCommands) -> Result<Output> {
    let write = !matches!(&command, RequestCommands::Get(_));
    let value = match command {
        RequestCommands::Get(args) => {
            let request = Request::get(&args.path).query(parse_key_values(&args.query)?);
            rt.with_session(|api| session_response(api, request.clone()))
                .await?
        }
        RequestCommands::Post(args) => raw_write(rt, Method::POST, args).await?,
        RequestCommands::Put(args) => raw_write(rt, Method::PUT, args).await?,
        RequestCommands::Delete(args) => {
            let request = Request::delete(&args.path).query(parse_key_values(&args.query)?);
            if args.confirm.is_dry_run() {
                dry_run_request(&request, "add --confirm to send the raw DELETE")
            } else {
                rt.with_session(|api| session_response(api, request.clone()))
                    .await
                    .map_err(|error| {
                        super::write_error(
                            error,
                            "check the affected account state before retrying",
                        )
                    })?
            }
        }
    };
    Ok(Output::view(value, move |value, doc| {
        if value["dry_run"] == true {
            doc.dry_run("", value);
        } else {
            if write
                && value["http_status"]
                    .as_u64()
                    .is_some_and(|status| (200..300).contains(&status))
            {
                doc.success("Request submitted");
            }
            render::generic(value, doc);
        }
    }))
}

async fn raw_write(rt: &Runtime, method: Method, args: RawWriteArgs) -> Result<Value> {
    let request = Request::new(method, &args.path)
        .query(parse_key_values(&args.query)?)
        .with_body(raw_body(&args)?);
    if args.confirm.is_dry_run() {
        Ok(dry_run_request(
            &request,
            "add --confirm to send the raw write request",
        ))
    } else {
        rt.with_session(|api| session_response(api, request.clone()))
            .await
            .map_err(|error| {
                super::write_error(error, "check the affected account state before retrying")
            })
    }
}

async fn session_response(api: KiteConnect, request: Request) -> kiteconnect::Result<Value> {
    let response = api.call(request).response().await?;
    let text = String::from_utf8_lossy(response.body());
    let body = serde_json::from_str::<Value>(&text).unwrap_or_else(|_| json!({ "body": text }));
    if matches!(text, std::borrow::Cow::Borrowed(_))
        && body.get("status").and_then(Value::as_str) == Some("error")
    {
        let message = body
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or_default();
        return Err(ApiError::new(
            ErrorKind::from(
                body.get("error_type")
                    .and_then(Value::as_str)
                    .unwrap_or("Unknown"),
            ),
            message,
            response.status(),
            body.get("data").cloned(),
        )
        .into());
    }
    Ok(
        json!({ "http_status": response.status(), "content_type": response.content_type().unwrap_or(""), "response": body }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    #[tokio::test]
    async fn raw_response_rejects_all_error_envelopes() {
        for error_type in ["TokenException", "InputException"] {
            let server = MockServer::start_async().await;
            let mock = server
                .mock_async(|when, then| {
                    when.method(POST).path("/orders/regular");
                    then.status(200).json_body(json!({
                        "status":"error", "error_type":error_type, "message":"rejected"
                    }));
                })
                .await;
            let client = KiteConnect::builder("test-key")
                .base_url(server.base_url())
                .build()
                .unwrap();
            let result = session_response(client, Request::post("/orders/regular")).await;
            if error_type == "TokenException" {
                let err = result.unwrap_err();
                assert!(err.is_session_expired());
                assert_eq!(
                    err.to_string(),
                    "kite API error TokenException (HTTP 200): rejected"
                );
            } else {
                let err = result.unwrap_err();
                assert!(!err.is_session_expired());
                assert_eq!(
                    err.to_string(),
                    "kite API error InputException (HTTP 200): rejected"
                );
            }
            mock.assert_calls_async(1).await;
        }
    }
    #[tokio::test]
    async fn raw_response_preserves_non_json_bodies_and_http_metadata() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET).path("/raw");
                then.status(200)
                    .header("content-type", "text/plain")
                    .body(vec![b'a', 0xff, b'b']);
            })
            .await;
        let api = KiteConnect::builder("test-key")
            .base_url(server.base_url())
            .build()
            .unwrap();
        assert_eq!(
            session_response(api, Request::get("/raw")).await.unwrap(),
            json!({"http_status":200,"content_type":"text/plain","response":{"body":"a�b"}})
        );
        mock.assert_calls_async(1).await;
    }
    #[tokio::test]
    async fn malformed_utf8_token_envelopes_remain_raw_responses() {
        let server = MockServer::start_async().await;
        let mock = server.mock_async(|when, then| {
            when.method(GET).path("/raw");
            then.status(200).body(b"{\"status\":\"error\",\"error_type\":\"TokenException\",\"message\":\"\xff\"}");
        }).await;
        let api = KiteConnect::builder("test-key")
            .base_url(server.base_url())
            .build()
            .unwrap();
        let value = session_response(api, Request::get("/raw")).await.unwrap();
        assert_eq!(
            value["response"],
            json!({"status":"error","error_type":"TokenException","message":"�"})
        );
        mock.assert_calls_async(1).await;
    }
}
