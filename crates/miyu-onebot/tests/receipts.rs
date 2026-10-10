//! 群里的命令回执几秒后撤回（施工 O-25 上，`onebot.md` 第一条「斜杠命令」第 7 条；18 第十节）：真核心拉起真桥、假 NapCat。
//! 群里的回执、被拒的那一句发出去 `receipt_recall_seconds`（出厂 3 秒）以后 `delete_msg` 撤回，撤的是 NapCat 回的那个编号；
//! 发命令的那条消息不撤；私聊的回执不撤。

use std::collections::BTreeSet;

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::group::*;
use crate::support::*;

/// 群号。
const GROUP: i64 = 888;

/// 群里的别人：用不了 `/stop`。
const LIN: i64 = 20002;

/// 假 NapCat 认得的群成员：她自己。
const MEMBERS: &[Member] = &[(BOT, "米尤", "miyu")];

/// 核心照中文写的 `/stop` 的回执（`resources/core/human/zh.json`）。
const STOPPED: &str = "已全部停下。";

/// 一个 `send_group_msg`、`send_private_msg` 动作里的字（只有一个文字段）。
fn text_of(action: &Value) -> &str {
    let message = action["params"]["message"].as_array().expect("段的数组");
    assert_eq!(message.len(), 1, "{action}");
    message[0]["data"]["text"].as_str().expect("有字")
}

#[tokio::test]
async fn group_receipts_are_recalled_and_private_ones_are_not() {
    let (home, mut napcat, _) = started(&Script::new([]), "", "", MEMBERS).await;
    // 私聊的 `/stop`：回执不撤。
    napcat.send(private_frame(ADMIN, 51, json!([plain("/stop")])));
    let private = napcat.action().await;
    assert_eq!(private["action"], "send_private_msg", "{private}");
    assert_eq!(text_of(&private), STOPPED);
    // 群里终端管理员的 `/stop`：回执撤；别人的 `/stop` 被拒，那一句也撤。
    napcat.send(group_frame(
        GROUP,
        ADMIN,
        52,
        json!([plain("/stop")]),
        ("终端管理员", "o"),
    ));
    let receipt = napcat.action().await;
    assert_eq!(text_of(&receipt), STOPPED, "{receipt}");
    napcat.send(group_frame(
        GROUP,
        LIN,
        53,
        json!([plain("/stop")]),
        ("小林", "lin"),
    ));
    let refused = napcat.action().await;
    assert_eq!(refused["action"], "send_group_msg", "{refused}");
    assert_ne!(text_of(&refused), STOPPED, "别人用不了");
    // 两个一样等 3 秒，谁先撤不一定：照编号比。
    let mut recalled = BTreeSet::new();
    for _ in 0..2 {
        let params = napcat.recalled().await;
        recalled.insert(params["message_id"].as_i64().expect("撤的是编号"));
    }
    assert_eq!(
        recalled,
        BTreeSet::from([FIRST_SENT + 1, FIRST_SENT + 2]),
        "撤的是群里那两句，私聊的回执不撤"
    );
    let log = until_log(&home, |log| log.matches("receipt recalled").count() == 2).await;
    assert!(
        napcat.pending_recall().is_none(),
        "发命令的那两条不撤：{log}"
    );
    // 回执、被拒的那一句都先入队（施工 O-25 中）：群里两句，私聊一句。
    let queued = |events: &[Value]| -> Vec<Value> {
        of_kind(events, "ext.onebot.venues.queued")
            .iter()
            .map(|one| one["body"].clone())
            .collect()
    };
    let group = queued(&venue_events(&home.root, &format!("qq:group:{GROUP}")));
    assert_eq!(
        group,
        [
            json!({"kind": "receipt", "text": STOPPED}),
            json!({"kind": "receipt", "text": text_of(&refused)}),
        ]
    );
    let private: Vec<Value> = home
        .sessions()
        .iter()
        .flat_map(|session| queued(&home.events(session)))
        .collect();
    assert_eq!(private, [json!({"kind": "receipt", "text": STOPPED})]);
    stopped(home).await;
}

/// 等到运行日志合 `wanted`：交回那时的运行日志。
async fn until_log(home: &Home, wanted: impl Fn(&str) -> bool) -> String {
    let deadline = tokio::time::Instant::now() + WAIT;
    loop {
        let log = run_log(&home.root);
        if wanted(&log) {
            return log;
        }
        assert!(tokio::time::Instant::now() < deadline, "等不到：{log}");
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}
