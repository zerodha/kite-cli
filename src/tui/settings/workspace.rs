use super::{Settings, read_document, set_paths};
use crate::tui::workspace::{PaneId, Preset};
use std::path::Path;

const WORKSPACES_VERSION: u64 = 2;

const LEGACY_VERSION: u64 = 1;

fn legacy_destination(token: &str) -> Option<PaneId> {
    Some(match token {
        "watch" => PaneId::TradingWatch,
        "positions" => PaneId::PortfolioPositions,
        "holdings" => PaneId::PortfolioHoldings,
        "orders" => PaneId::OrdersBook,
        "gtt" => PaneId::OrdersGtt,
        "alerts" => PaneId::OrdersAlerts,
        "mf" => PaneId::PortfolioMf,
        "bids" => PaneId::InvestBids,
        "chain" => PaneId::DerivativesChain,
        _ => return None,
    })
}

fn migrate_legacy_account(
    account: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Map<String, serde_json::Value> {
    let mut migrated = account.clone();
    for field in ["focused", "maximized"] {
        if let Some(map) = migrated
            .get_mut(field)
            .and_then(serde_json::Value::as_object_mut)
        {
            map.retain(|workspace, _| legacy_destination(workspace).is_none());
        }
    }
    let Some(pane) = account
        .get("active")
        .and_then(serde_json::Value::as_str)
        .and_then(legacy_destination)
    else {
        return migrated;
    };
    let preset = pane.preset().config_token();
    migrated.insert("active".to_string(), serde_json::json!(preset));
    for (field, value) in [
        ("focused", serde_json::json!(pane.config_token())),
        ("maximized", serde_json::Value::Null),
    ] {
        let entry = migrated
            .entry(field)
            .or_insert_with(|| serde_json::json!({}));
        if !entry.is_object() {
            *entry = serde_json::json!({});
        }
        if let Some(map) = entry.as_object_mut() {
            map.insert(preset.to_string(), value);
        }
    }
    migrated
}

const WORKSPACES_KEY: &str = "workspaces";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspacePreferences {
    pub active: Option<Preset>,
    pub focused: Vec<(Preset, PaneId)>,
    pub maximized: Vec<(Preset, PaneId)>,
}

impl Settings {
    pub fn workspace_preferences(&self, user_id: &str) -> WorkspacePreferences {
        let mut prefs = WorkspacePreferences::default();
        if user_id.is_empty() {
            return prefs;
        }
        let Some(root) = self
            .workspaces
            .as_ref()
            .and_then(serde_json::Value::as_object)
        else {
            return prefs;
        };
        let legacy = match root.get("version").and_then(serde_json::Value::as_u64) {
            Some(WORKSPACES_VERSION) => false,
            Some(LEGACY_VERSION) => true,
            _ => return prefs,
        };
        let Some(account) = root
            .get("accounts")
            .and_then(serde_json::Value::as_object)
            .and_then(|accounts| accounts.get(user_id))
            .and_then(serde_json::Value::as_object)
        else {
            return prefs;
        };
        let migrated;
        let account = if legacy {
            migrated = migrate_legacy_account(account);
            &migrated
        } else {
            account
        };
        prefs.active = account
            .get("active")
            .and_then(serde_json::Value::as_str)
            .and_then(Preset::from_config_token);
        prefs.focused = panes_by_preset(account.get("focused"));
        prefs.maximized = panes_by_preset(account.get("maximized"));
        prefs
    }
}

fn panes_by_preset(raw: Option<&serde_json::Value>) -> Vec<(Preset, PaneId)> {
    let Some(map) = raw.and_then(serde_json::Value::as_object) else {
        return Vec::new();
    };
    map.iter()
        .filter_map(|(workspace, pane)| {
            let preset = Preset::from_config_token(workspace)?;
            let pane = PaneId::from_config_token(preset, pane.as_str()?)?;
            Some((preset, pane))
        })
        .collect()
}

pub fn save_workspace_preferences(
    path: &Path,
    user_id: &str,
    workspace: Preset,
) -> Result<(), String> {
    save_workspace_settings_at(
        path,
        vec![(
            &[WORKSPACES_KEY, "accounts", user_id, "active"],
            serde_json::json!(workspace.config_token()),
        )],
    )
}

pub fn save_workspace_focus(
    path: &Path,
    user_id: &str,
    workspace: Preset,
    focused: PaneId,
    maximized: Option<PaneId>,
) -> Result<(), String> {
    save_workspace_settings_at(
        path,
        vec![
            (
                &[
                    WORKSPACES_KEY,
                    "accounts",
                    user_id,
                    "focused",
                    workspace.config_token(),
                ],
                serde_json::json!(focused.config_token()),
            ),
            (
                &[
                    WORKSPACES_KEY,
                    "accounts",
                    user_id,
                    "maximized",
                    workspace.config_token(),
                ],
                match maximized {
                    Some(pane) => serde_json::json!(pane.config_token()),
                    None => serde_json::Value::Null,
                },
            ),
        ],
    )
}

fn save_workspace_settings_at(
    path: &Path,
    mut writes: Vec<(&[&str], serde_json::Value)>,
) -> Result<(), String> {
    let mut document = read_document(path)?;
    let subtree = &document[WORKSPACES_KEY];
    if let Some(version) = subtree.get("version") {
        match version.as_u64() {
            Some(WORKSPACES_VERSION) => {}
            Some(LEGACY_VERSION) => {
                if let Some(accounts) = document[WORKSPACES_KEY]["accounts"].as_object_mut() {
                    for entry in accounts.values_mut() {
                        if let Some(account) = entry.as_object() {
                            *entry = serde_json::Value::Object(migrate_legacy_account(account));
                        }
                    }
                }
            }
            _ => {
                return Err(format!(
                    "{} holds workspaces version {version}, which this build does not write",
                    path.display()
                ));
            }
        }
    }
    writes.insert(
        0,
        (
            &[WORKSPACES_KEY, "version"],
            serde_json::json!(WORKSPACES_VERSION),
        ),
    );
    set_paths(&mut document, writes);
    crate::util::save_json_atomic(path, &document)
}

#[cfg(test)]
mod tests {
    use super::super::from_json;
    use super::super::tests::scratch;
    use serde_json::json;

    use crate::tui::workspace::{PaneId, Preset};

    #[test]
    fn an_old_document_remembers_no_workspace() {
        let settings = from_json(r#"{"theme":"dracula","selected_watch":{"AB1234":"W7"}}"#);
        assert_eq!(
            settings.workspace_preferences("AB1234"),
            super::WorkspacePreferences::default()
        );
        assert_eq!(settings.theme(), super::super::preset("dracula").unwrap());
        assert_eq!(settings.selected_watch("AB1234"), Some("W7"));
    }

    #[test]
    fn a_malformed_workspaces_value_drops_only_itself() {
        for raw in [
            r#"{"theme":"dracula","workspaces":"trading"}"#,
            r#"{"theme":"dracula","workspaces":[1,2,3]}"#,
            r#"{"theme":"dracula","workspaces":null}"#,
            r#"{"theme":"dracula","workspaces":{"version":2,"accounts":"AB1234"}}"#,
            r#"{"theme":"dracula","workspaces":{"version":2,"accounts":{"AB1234":7}}}"#,
            r#"{"theme":"dracula","workspaces":{"version":"1","accounts":{"AB1234":{"active":"trading"}}}}"#,
        ] {
            let settings = from_json(raw);
            assert_eq!(
                settings.workspace_preferences("AB1234"),
                super::WorkspacePreferences::default(),
                "{raw}"
            );
            assert_eq!(
                settings.theme(),
                super::super::preset("dracula").unwrap(),
                "a bad workspace value must not change the theme: {raw}"
            );
        }
    }

    #[test]
    fn an_unknown_version_is_ignored_but_kept() {
        let raw = r#"{"workspaces":{"version":3,"accounts":{"AB1234":{"active":"trading"}}}}"#;
        let settings = from_json(raw);
        assert_eq!(
            settings.workspace_preferences("AB1234"),
            super::WorkspacePreferences::default()
        );
        assert_eq!(
            settings.workspaces,
            Some(
                serde_json::from_str::<serde_json::Value>(
                    r#"{"version":3,"accounts":{"AB1234":{"active":"trading"}}}"#
                )
                .unwrap()
            )
        );
    }

    #[test]
    fn an_unknown_id_drops_only_its_own_entry() {
        let settings = from_json(
            r#"{"workspaces":{"version":2,"accounts":{"AB1234":{
                "active":"nowhere",
                "focused":{"trading":"trading.orders","portfolio":"trading.watch",
                           "atlantis":"trading.watch","derivatives":42},
                "maximized":{"trading":"trading.chart","overview":"overview.nothing"}
            }}}}"#,
        );
        let prefs = settings.workspace_preferences("AB1234");
        assert_eq!(
            prefs.active, None,
            "an unknown workspace opens the Overview"
        );
        assert_eq!(
            prefs.focused,
            vec![(Preset::Trading, PaneId::TradingOrders)],
            "only the entry naming a pane its own workspace draws survives"
        );
        assert_eq!(
            prefs.maximized,
            vec![(Preset::Trading, PaneId::TradingChart)]
        );
    }

    #[test]
    fn mf_saved_under_invest_drops_and_its_portfolio_entry_restores() {
        let settings = from_json(
            r#"{"workspaces":{"version":2,"accounts":{"AB1234":{
                "active":"invest",
                "focused":{"invest":"invest.mf","portfolio":"portfolio.mf"},
                "maximized":{"invest":"invest.mf"}
            }}}}"#,
        );
        let prefs = settings.workspace_preferences("AB1234");
        assert_eq!(prefs.active, Some(Preset::Invest));
        assert_eq!(
            prefs.focused,
            vec![(Preset::Portfolio, PaneId::PortfolioMf)]
        );
        assert_eq!(prefs.maximized, Vec::new());
    }

    #[test]
    fn workspace_preferences_are_keyed_by_account() {
        let settings = from_json(
            r#"{"workspaces":{"version":2,"accounts":{
                "AB1234":{"active":"trading","focused":{"trading":"trading.depth"}},
                "CD5678":{"active":"portfolio"}
            }}}"#,
        );
        let first = settings.workspace_preferences("AB1234");
        assert_eq!(first.active, Some(Preset::Trading));
        assert_eq!(first.focused, vec![(Preset::Trading, PaneId::TradingDepth)]);

        let second = settings.workspace_preferences("CD5678");
        assert_eq!(second.active, Some(Preset::Portfolio));
        assert_eq!(second.focused.len(), 0);

        assert_eq!(
            settings.workspace_preferences("ZZ0000"),
            super::WorkspacePreferences::default()
        );
        assert_eq!(
            settings.workspace_preferences(""),
            super::WorkspacePreferences::default()
        );
    }

    #[test]
    fn a_saved_workspace_reads_back_and_keeps_every_other_field() {
        let path = scratch("workspaces");
        std::fs::write(
            &path,
            r#"{"theme":"dracula","hand_written":{"keep":true},
                "workspaces":{"future_field":["kept"],
                              "accounts":{"CD5678":{"active":"derivatives"}}}}"#,
        )
        .expect("write");

        super::save_workspace_preferences(&path, "AB1234", Preset::Trading).expect("save");
        super::save_workspace_focus(
            &path,
            "AB1234",
            Preset::Trading,
            PaneId::TradingOrders,
            Some(PaneId::TradingOrders),
        )
        .expect("save");

        let raw = std::fs::read_to_string(&path).expect("read");
        let settings = from_json(&raw);
        let prefs = settings.workspace_preferences("AB1234");
        assert_eq!(prefs.active, Some(Preset::Trading));
        assert_eq!(
            prefs.focused,
            vec![(Preset::Trading, PaneId::TradingOrders)]
        );
        assert_eq!(
            prefs.maximized,
            vec![(Preset::Trading, PaneId::TradingOrders)]
        );

        assert_eq!(
            settings.workspace_preferences("CD5678").active,
            Some(Preset::Derivatives)
        );
        assert_eq!(settings.theme(), super::super::preset("dracula").unwrap());
        let document: serde_json::Value = serde_json::from_str(&raw).expect("json");
        assert_eq!(document["hand_written"]["keep"], json!(true));
        assert_eq!(document["workspaces"]["future_field"], json!(["kept"]));
        assert_eq!(document["workspaces"]["version"], json!(2));

        super::save_workspace_focus(&path, "AB1234", Preset::Trading, PaneId::TradingWatch, None)
            .expect("save");
        let prefs = from_json(&std::fs::read_to_string(&path).expect("read"))
            .workspace_preferences("AB1234");
        assert_eq!(prefs.focused, vec![(Preset::Trading, PaneId::TradingWatch)]);
        assert_eq!(prefs.maximized.len(), 0);
    }

    #[test]
    fn a_failed_workspace_save_leaves_the_file_alone() {
        let path = scratch("workspaces-malformed");
        let original = r#"{"theme":"dracula",}"#;
        std::fs::write(&path, original).expect("write");

        let err = super::save_workspace_preferences(&path, "AB1234", Preset::Trading)
            .expect_err("a document that cannot be read cannot be written back");
        assert!(err.contains("not valid JSON"), "{err}");
        let err = super::save_workspace_focus(
            &path,
            "AB1234",
            Preset::Trading,
            PaneId::TradingWatch,
            None,
        )
        .expect_err("the focus write follows the same rule");
        assert!(err.contains("not valid JSON"), "{err}");
        assert_eq!(std::fs::read_to_string(&path).expect("read"), original);
    }

    #[test]
    fn a_workspace_save_refuses_an_unsupported_version() {
        let path = scratch("workspaces-future");
        let original = r#"{"theme":"dracula","workspaces":{"version":3,
            "accounts":{"CD5678":{"active":"derivatives"}}}}"#;
        std::fs::write(&path, original).expect("write");

        let err = super::save_workspace_preferences(&path, "AB1234", Preset::Trading)
            .expect_err("a newer schema is not overwritten");
        assert!(err.contains("workspaces version 3"), "{err}");
        let err = super::save_workspace_focus(
            &path,
            "AB1234",
            Preset::Trading,
            PaneId::TradingWatch,
            None,
        )
        .expect_err("the focus write follows the same rule");
        assert!(err.contains("workspaces version 3"), "{err}");

        let raw = std::fs::read_to_string(&path).expect("read");
        assert_eq!(raw, original);
        assert_eq!(
            from_json(&raw).workspace_preferences("CD5678").active,
            None,
            "the other account's entry stays unreadable to this build"
        );
    }

    #[test]
    fn a_non_object_workspaces_field_is_replaced_by_the_write() {
        let path = scratch("workspaces-scalar");
        std::fs::write(&path, r#"{"theme":"dracula","workspaces":"trading"}"#).expect("write");
        super::save_workspace_preferences(&path, "AB1234", Preset::Portfolio).expect("save");
        let settings = from_json(&std::fs::read_to_string(&path).expect("read"));
        assert_eq!(
            settings.workspace_preferences("AB1234").active,
            Some(Preset::Portfolio)
        );
        assert_eq!(settings.theme(), super::super::preset("dracula").unwrap());
    }

    #[test]
    fn a_version_one_single_view_migrates_to_the_workspace_holding_its_pane() {
        for (token, preset, pane) in [
            ("watch", Preset::Trading, PaneId::TradingWatch),
            ("positions", Preset::Portfolio, PaneId::PortfolioPositions),
            ("holdings", Preset::Portfolio, PaneId::PortfolioHoldings),
            ("orders", Preset::Orders, PaneId::OrdersBook),
            ("gtt", Preset::Orders, PaneId::OrdersGtt),
            ("alerts", Preset::Orders, PaneId::OrdersAlerts),
            ("mf", Preset::Portfolio, PaneId::PortfolioMf),
            ("bids", Preset::Invest, PaneId::InvestBids),
            ("chain", Preset::Derivatives, PaneId::DerivativesChain),
        ] {
            let settings = from_json(&format!(
                r#"{{"workspaces":{{"version":1,"accounts":{{"AB1234":{{
                    "active":"{token}",
                    "focused":{{"{token}":"{token}.full","trading":"trading.chart",
                               "portfolio":"portfolio.funds"}},
                    "maximized":{{"{token}":"{token}.full","portfolio":"portfolio.funds"}}
                }}}}}}}}"#
            ));
            let prefs = settings.workspace_preferences("AB1234");
            assert_eq!(prefs.active, Some(preset), "{token}");
            assert!(
                prefs.focused.contains(&(preset, pane)),
                "{token}: {prefs:?}"
            );
            assert!(
                !prefs.maximized.iter().any(|(saved, _)| *saved == preset),
                "{token}: nothing is maximized over the migrated pane: {prefs:?}"
            );
            if preset != Preset::Trading {
                assert!(
                    prefs
                        .focused
                        .contains(&(Preset::Trading, PaneId::TradingChart)),
                    "{token}: {prefs:?}"
                );
            }
            if preset != Preset::Portfolio {
                assert!(
                    prefs
                        .maximized
                        .contains(&(Preset::Portfolio, PaneId::PortfolioFunds)),
                    "{token}: {prefs:?}"
                );
            }
        }
        let overview = from_json(
            r#"{"workspaces":{"version":1,"accounts":{"AB1234":{"active":"overview"}}}}"#,
        );
        assert_eq!(
            overview.workspace_preferences("AB1234").active,
            Some(Preset::Overview)
        );
    }

    #[test]
    fn a_write_to_a_version_one_file_migrates_every_account() {
        let path = scratch("workspaces-legacy");
        std::fs::write(
            &path,
            r#"{"theme":"dracula","workspaces":{"version":1,"accounts":{
                "AB1234":{"active":"watch"},
                "CD5678":{"active":"gtt","focused":{"gtt":"gtt.full"}}
            }}}"#,
        )
        .expect("write");
        super::save_workspace_focus(&path, "AB1234", Preset::Trading, PaneId::TradingDepth, None)
            .expect("a version 1 file takes the write");

        let raw = std::fs::read_to_string(&path).expect("read");
        let document: serde_json::Value = serde_json::from_str(&raw).expect("json");
        assert_eq!(document["workspaces"]["version"], json!(2));
        let settings = from_json(&raw);
        let first = settings.workspace_preferences("AB1234");
        assert_eq!(first.active, Some(Preset::Trading));
        assert_eq!(first.focused, vec![(Preset::Trading, PaneId::TradingDepth)]);
        let second = settings.workspace_preferences("CD5678");
        assert_eq!(second.active, Some(Preset::Orders));
        assert_eq!(second.focused, vec![(Preset::Orders, PaneId::OrdersGtt)]);
        assert_eq!(settings.theme(), super::super::preset("dracula").unwrap());
    }
}
