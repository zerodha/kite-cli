use super::{Catalog, Detail, GroupId, ItemId, Mutation, WatchId};
use crate::tui::data::api_err;
use kiteconnect::KiteConnect;
use kiteconnect::Request;
use serde::Deserialize;

const MARKETWATCH_VERSION: &str = "3.2.0";

async fn call(client: &KiteConnect, request: Request) -> Result<serde_json::Value, String> {
    client
        .call(request.kite_version(MARKETWATCH_VERSION))
        .raw()
        .await
        .map_err(api_err)
}

pub async fn fetch_catalog(client: &KiteConnect) -> Result<Catalog, String> {
    let value = call(client, Request::get("/api/marketwatch")).await?;
    serde_json::from_value(value).map_err(|err| format!("bad marketwatch list payload: {err}"))
}

pub async fn fetch_detail(client: &KiteConnect, watch: &WatchId) -> Result<Detail, String> {
    let value = call(client, Request::get(items_path(watch))).await?;
    serde_json::from_value(value).map_err(|err| format!("bad marketwatch items payload: {err}"))
}

async fn add_item(
    client: &KiteConnect,
    watch: &WatchId,
    exchange: &str,
    tradingsymbol: &str,
    weight: f64,
    group: &GroupId,
) -> Result<(), String> {
    let form = vec![
        ("exchange".to_string(), exchange.to_string()),
        ("tradingsymbol".to_string(), tradingsymbol.to_string()),
        ("weight".to_string(), weight.to_string()),
        ("group".to_string(), group.to_string()),
    ];
    call(client, Request::post(items_path(watch)).form(form))
        .await
        .map(drop)
}

async fn delete_item(client: &KiteConnect, watch: &WatchId, item: &ItemId) -> Result<(), String> {
    call(
        client,
        Request::delete(format!("{}/{item}", items_path(watch))),
    )
    .await
    .map(drop)
}

async fn reorder_items(
    client: &KiteConnect,
    watch: &WatchId,
    form: Vec<(String, String)>,
) -> Result<(), String> {
    call(client, Request::put(items_path(watch)).form(form))
        .await
        .map(drop)
}

async fn create_watch(client: &KiteConnect, name: &str, weight: usize) -> Result<WatchId, String> {
    #[derive(Deserialize)]
    struct Created {
        id: WatchId,
    }
    let form = vec![
        ("name".to_string(), name.to_string()),
        ("weight".to_string(), weight.to_string()),
    ];
    let value = call(client, Request::post("/api/marketwatch").form(form)).await?;
    serde_json::from_value::<Created>(value)
        .map(|created| created.id)
        .map_err(|err| format!("created watchlist but the server returned no id: {err}"))
}

async fn rename_watch(client: &KiteConnect, watch: &WatchId, name: &str) -> Result<(), String> {
    call(
        client,
        Request::put(format!("/api/marketwatch/{watch}"))
            .form(vec![("name".to_string(), name.to_string())]),
    )
    .await
    .map(drop)
}

async fn delete_watch(client: &KiteConnect, watch: &WatchId) -> Result<(), String> {
    call(client, Request::delete(format!("/api/marketwatch/{watch}")))
        .await
        .map(drop)
}

fn items_path(watch: &WatchId) -> String {
    format!("/api/marketwatch/{watch}/items")
}

pub async fn apply(client: &KiteConnect, mutation: &Mutation) -> Result<Option<WatchId>, String> {
    match mutation {
        Mutation::Add {
            watch,
            exchange,
            tradingsymbol,
            weight,
            group,
            ..
        } => add_item(client, watch, exchange, tradingsymbol, *weight, group)
            .await
            .map(|()| None),
        Mutation::Remove { watch, item, .. } => {
            delete_item(client, watch, item).await.map(|()| None)
        }
        Mutation::Reorder { watch, form } => reorder_items(client, watch, form.clone())
            .await
            .map(|()| None),
        Mutation::Create { name, weight } => create_watch(client, name, *weight).await.map(Some),
        Mutation::Rename { watch, name } => rename_watch(client, watch, name).await.map(|()| None),
        Mutation::Delete { watch, .. } => delete_watch(client, watch).await.map(|()| None),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::marketwatch::tests::{catalog_body, client, detail_body, response};
    use httpmock::prelude::*;
    use serde_json::json;

    #[tokio::test]
    async fn reads_go_to_api_paths_with_token_auth() {
        let server = MockServer::start();
        let catalog = server.mock(|when, then| {
            when.method(GET)
                .path("/api/marketwatch")
                .header("Authorization", "token key:token")
                .header("X-Kite-Version", "3.2.0")
                .header_missing("cookie")
                .header_missing("X-CSRFTOKEN");
            then.status(200).body(response(&catalog_body()));
        });
        let items = server.mock(|when, then| {
            when.method(GET)
                .path("/api/marketwatch/W1/items")
                .header("X-Kite-Version", "3.2.0");
            then.status(200).body(response(&detail_body()));
        });

        let client = client(&server);
        let parsed = fetch_catalog(&client).await.unwrap();
        assert_eq!(parsed.sections.len(), 2);
        assert_eq!(parsed.sections[0].items[0].id.as_str(), "W1");
        assert!(!parsed.sections[0].items[0].is_static);
        assert!(parsed.sections[0].items[1].is_static);
        assert_eq!(parsed.sections[1].items[0].name, "Long term");

        let detail = fetch_detail(&client, &WatchId::from("W1")).await.unwrap();
        assert_eq!(detail.name, "Core");
        assert_eq!(detail.groups.len(), 2);
        let rows = detail.rows();
        assert_eq!(rows.len(), 3, "groups flatten into one display list");
        assert_eq!(rows[0].id, ItemId::from("ITEM:a"));
        assert_eq!(rows[0].group, GroupId::from("GROUP:default"));
        assert_eq!(rows[2].tradingsymbol, "NIFTY 50");
        assert_eq!(rows[2].group, GroupId::from("GROUP:b"));
        assert!(rows[2].is_index());
        catalog.assert();
        items.assert();
    }

    #[tokio::test]
    async fn writes_send_the_forms_kite_web_sends() {
        let server = MockServer::start();
        let add = server.mock(|when, then| {
            when.method(POST)
                .path("/api/marketwatch/W1/items")
                .header("Authorization", "token key:token")
                .header("X-Kite-Version", "3.2.0")
                .body("exchange=NSE&tradingsymbol=INFY&weight=1.0001&group=GROUP%3Adefault");
            then.status(200).body(response(&json!({})));
        });
        let remove = server.mock(|when, then| {
            when.method(DELETE)
                .path("/api/marketwatch/W1/items/ITEM:a")
                .header("X-Kite-Version", "3.2.0");
            then.status(200).body(response(&json!(null)));
        });
        let reorder = server.mock(|when, then| {
            when.method(PUT)
                .path("/api/marketwatch/W1/items")
                .header("X-Kite-Version", "3.2.0")
                .body("GROUP%3Adefault=1&ITEM%3Aa=2&ITEM%3Ab=3&GROUP%3Ab=4&ITEM%3Ac=5");
            then.status(200).body(response(&json!({})));
        });
        let create = server.mock(|when, then| {
            when.method(POST)
                .path("/api/marketwatch")
                .header("X-Kite-Version", "3.2.0")
                .body("name=Swing&weight=2");
            then.status(200).body(response(&json!({"id": "W9"})));
        });
        let rename = server.mock(|when, then| {
            when.method(PUT)
                .path("/api/marketwatch/W1")
                .header("X-Kite-Version", "3.2.0")
                .body("name=Long+term");
            then.status(200).body(response(&json!({})));
        });
        let delete = server.mock(|when, then| {
            when.method(DELETE)
                .path("/api/marketwatch/W1")
                .header("X-Kite-Version", "3.2.0");
            then.status(200).body(response(&json!({})));
        });

        let client = client(&server);
        let watch = WatchId::from("W1");
        let detail: Detail = serde_json::from_value(detail_body()).unwrap();

        add_item(
            &client,
            &watch,
            "NSE",
            "INFY",
            detail.groups[0].items.first().unwrap().weight,
            &GroupId::from("GROUP:default"),
        )
        .await
        .unwrap();
        delete_item(&client, &watch, &ItemId::from("ITEM:a"))
            .await
            .unwrap();
        reorder_items(&client, &watch, detail.reorder_form())
            .await
            .unwrap();
        assert_eq!(
            create_watch(&client, "Swing", 2).await.unwrap(),
            WatchId::from("W9")
        );
        rename_watch(&client, &watch, "Long term").await.unwrap();
        delete_watch(&client, &watch).await.unwrap();

        add.assert();
        remove.assert();
        reorder.assert();
        create.assert();
        rename.assert();
        delete.assert();
    }
    #[tokio::test]
    async fn a_malformed_payload_is_an_error_not_an_empty_watchlist() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/api/marketwatch");
            then.status(200)
                .body(response(&json!({"not": "an array of sections"})));
        });
        let err = fetch_catalog(&client(&server)).await.unwrap_err();
        assert!(err.contains("bad marketwatch list payload"), "{err}");
    }

    #[tokio::test]
    async fn a_token_exception_surfaces_as_an_auth_error() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/api/marketwatch");
            then.status(403).body(
                json!({"status": "error", "message": "Invalid or expired session.",
                       "error_type": "TokenException"})
                .to_string(),
            );
        });
        let err = fetch_catalog(&client(&server)).await.unwrap_err();
        assert!(crate::app::errors::is_auth_error_text(&err), "{err}");
        assert!(err.contains("Invalid or expired session."), "{err}");
    }

    #[tokio::test]
    async fn a_server_error_message_reaches_the_caller_verbatim() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/api/marketwatch/W1/items");
            then.status(400).body(
                json!({"status": "error", "message": "Maximum 250 instruments per watchlist.",
                       "error_type": "InputException"})
                .to_string(),
            );
        });
        let err = add_item(
            &client(&server),
            &WatchId::from("W1"),
            "NSE",
            "INFY",
            1.0001,
            &GroupId::from("GROUP:default"),
        )
        .await
        .unwrap_err();
        assert!(
            err.contains("Maximum 250 instruments per watchlist."),
            "{err}"
        );
    }

    #[tokio::test]
    async fn a_create_without_an_id_is_an_error() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/api/marketwatch");
            then.status(200).body(response(&json!({})));
        });
        let err = create_watch(&client(&server), "Swing", 1)
            .await
            .unwrap_err();
        assert!(err.contains("returned no id"), "{err}");
    }
}
