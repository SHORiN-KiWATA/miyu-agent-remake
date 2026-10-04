//! 配置页的异步请求与连接丢失处理（蓝图 tui.md「全屏配置页」第 7、8 条）。

use super::{Update, awaiting::Awaiting};
use serde_json::Value;
use std::collections::HashMap;

/// 回应只交给配置页，未来新增 result 字段也照原样保留。
pub(super) fn reply(tag: u64, message: &Value) -> Update {
    let result = match message.get("error") {
        Some(error) => Err(error["message"].as_str().unwrap_or_default().to_string()),
        None => Ok(message["result"].clone()),
    };
    Update::SettingsRpc { tag, result }
}

/// 连接失效不重放写请求，告知配置页结果未知，以免误报成功。
pub(super) fn disconnected(tag: u64) -> Update {
    Update::SettingsRpc {
        tag,
        result: Err(std::io::Error::new(
            std::io::ErrorKind::ConnectionAborted,
            "Connection lost before response; request was not replayed.",
        )
        .to_string()),
    }
}

/// 丢弃失效连接的等待表，每个配置请求回失败一次。
pub(super) fn lost(awaiting: &mut HashMap<String, Awaiting>, notify: &impl Fn(Update) -> bool) {
    for kind in awaiting.drain().map(|(_, kind)| kind) {
        if let Awaiting::SettingsRpc(tag) = kind {
            notify(disconnected(tag));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Command, Update, asides, awaiting::Awaiting};
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn settings_requests_keep_the_ui_tag_and_do_not_need_a_session() {
        let params = json!({"layer":"personal","secrets":{"example":"secret-test"}});
        let command = Command::SettingsRpc {
            tag: 42,
            method: "config.set",
            params: params.clone(),
        };
        let (method, got, kind) = asides::request(&command).unwrap();
        assert_eq!(method, "config.set");
        assert_eq!(got, params);
        assert_eq!(kind, Some(Awaiting::SettingsRpc(42)));
        assert!(!format!("{command:?}").contains("secret-test"));
    }

    #[test]
    fn replies_deliver_raw_results_and_core_refusals_once() {
        let raw = json!({"revision":"v2","unknown_future_field":[1,2]});
        match reply(9, &json!({"result":raw})) {
            Update::SettingsRpc { tag, result } => {
                assert_eq!(tag, 9);
                assert_eq!(result.unwrap(), raw);
            }
            _ => panic!("wrong update"),
        }
        match reply(
            10,
            &json!({"error":{"message":"版本冲突","data":{"reason":"config_conflict"}}}),
        ) {
            Update::SettingsRpc { tag, result } => {
                assert_eq!(tag, 10);
                assert_eq!(result.unwrap_err(), "版本冲突");
            }
            _ => panic!("wrong update"),
        }
    }

    #[test]
    fn connection_loss_finishes_pending_settings_without_replaying() {
        let mut pending = HashMap::from([
            ("read".into(), Awaiting::SettingsRpc(7)),
            ("write".into(), Awaiting::SettingsRpc(8)),
            ("models".into(), Awaiting::Models),
        ]);
        let updates = std::cell::RefCell::new(Vec::new());
        lost(&mut pending, &|u| {
            updates.borrow_mut().push(u);
            true
        });
        assert!(pending.is_empty());
        lost(&mut pending, &|u| {
            updates.borrow_mut().push(u);
            true
        });
        let tags: Vec<u64> = updates
            .into_inner()
            .into_iter()
            .map(|u| match u {
                Update::SettingsRpc { tag, result } => {
                    assert!(result.is_err());
                    tag
                }
                _ => panic!("wrong update"),
            })
            .collect();
        assert_eq!(tags.len(), 2);
        assert!(tags.contains(&7) && tags.contains(&8));
    }

    #[test]
    fn standalone_config_never_selects_a_resume_or_recent_session() {
        assert_eq!(
            super::super::initial_session(Some("explicit"), true, true),
            (None, false)
        );
        assert_eq!(
            super::super::initial_session(None, true, true),
            (None, false)
        );
        assert_eq!(
            super::super::initial_session(Some("explicit"), true, false),
            (Some("explicit"), true)
        );
    }
}
