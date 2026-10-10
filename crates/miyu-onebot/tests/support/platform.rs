//! 平台工具（一）的测试共用的（施工 O-31，`tests/platform.rs`）：一个不抽样的群、几种人（冒充的、管理的人、群主、白名单成员、
//! QQ 的管理员），她调一件工具、接着说一句的台词，开一轮、等它完、读工具结果，看 NapCat 收到的平台动作。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::request::Request;

use super::group::*;
use super::speaking::{Line, Lines};
use super::*;

/// 不抽样的群：别人的话只记下，终端管理员 @ 她开一轮。
pub const GROUP: i64 = 31;

/// 普通成员。
pub const MEMBER: i64 = 40001;

/// 另一个普通成员：群名片写成「终端管理员」冒充。
pub const IMPOSTOR: i64 = 40002;

/// 场所规则里管理的人（`managers`）。
pub const MANAGER: i64 = 40003;

/// 群主：说话不带身份（缓存里没有，像记下的过了时），身份要问 NapCat。
pub const OWNER: i64 = 40004;

/// 白名单成员。
pub const FRIEND: i64 = 40005;

/// QQ 的群管理员：说过话，身份照 `sender.role` 记下；问 NapCat 的话是普通成员（不在 [`RANKS`] 里），挡住他的只能是记下的那一份。
pub const QQ_ADMIN: i64 = 40006;

/// 终端管理员的小号：对应表里有（`support/mod.rs` 的系统配置），没在群里说过话（施工 O-31）。
pub const ALT: i64 = 10007;

/// 系统的场所规则：群都不抽样，[`MANAGER`] 是管理的人。
pub fn rules() -> String {
    format!(
        "[[rule]]\nmatch = {{ kind = \"group\" }}\nchatty = {{ probability = 0 }}\nmanagers = [\"qq:{MANAGER}\"]\n"
    )
}

/// 系统配置的 `[onebot]`：[`FRIEND`] 是白名单成员。
pub fn onebot() -> String {
    format!("whitelist = [\"qq:{FRIEND}\"]\n")
}

/// 假 NapCat 认得的群成员。
pub const MEMBERS: &[Member] = &[
    (BOT, "米尤", "miyu"),
    (ADMIN, "老板", "boss"),
    (MEMBER, "小明", "ming"),
    (IMPOSTOR, "终端管理员", "fake"),
    (MANAGER, "群管", "guan"),
    (OWNER, "群主", "zhu"),
    (FRIEND, "小杰", "jie"),
    (QQ_ADMIN, "管理员", "admin"),
    (ALT, "小号", "alt"),
];

/// 问群成员时回的身份。
pub const RANKS: &[Rank] = &[(OWNER, "owner")];

/// 照 NapCat 回、桥调的平台动作。
pub const PLATFORM: [&str; 4] = ["delete_msg", "set_group_ban", "group_poke", "friend_poke"];

/// 这个群的场所编号。
pub fn venue() -> String {
    format!("qq:group:{GROUP}")
}

/// 群名片、昵称照 [`MEMBERS`]。
pub fn names(user: i64) -> (&'static str, &'static str) {
    MEMBERS
        .iter()
        .find(|(id, _, _)| *id == user)
        .map(|(_, card, nickname)| (*card, *nickname))
        .expect("认得的成员")
}

/// 她调一件工具（不说话），这一轮接着说一句「好。」：一轮两句台词。
pub fn uses(tool: &'static str, args: &'static str) -> [Line; 2] {
    [Line::uses("", tool, args), Line::says("好。")]
}

/// 起核心、桥、假 NapCat：她照 `lines` 说。
pub async fn up(lines: &Lines) -> (Home, Answering) {
    let (home, napcat, _) = started_ranked(
        Arc::new(lines.clone()),
        &rules(),
        &onebot(),
        (MEMBERS, RANKS),
    )
    .await;
    (home, napcat)
}

/// `user` 在群里说第 `message` 条（`words` 是段），身份 `role`，等它记下：交回它的序号。
pub async fn says(
    home: &Home,
    napcat: &Answering,
    (user, role): (i64, &str),
    message: i64,
    words: Value,
) -> u64 {
    napcat.send(group_frame_as(
        GROUP,
        user,
        message,
        words,
        names(user),
        role,
    ));
    let msg = message.to_string();
    let events = until_event(&home.root, &venue(), |event| {
        event["kind"] == "message.user" && event["body"]["venue"]["msg"] == msg.as_str()
    })
    .await;
    said(&events)
        .iter()
        .find(|event| event["body"]["venue"]["msg"] == msg.as_str())
        .and_then(|event| event["seq"].as_u64())
        .expect("有序号")
}

/// 普通成员 `user` 说第 `message` 条，照判官点了头的样子拿它开第 `turn` 轮，等这一轮完：交回这一轮的工具结果（字，状态）。
pub async fn asks(
    home: &Home,
    napcat: &Answering,
    user: i64,
    message: i64,
    words: Value,
    turn: usize,
) -> (String, String) {
    let seq = says(home, napcat, (user, "member"), message, words).await;
    respond(home, &venue(), &format!("open-{message}"), &[json!(seq)]).await;
    ended(home, turn).await
}

/// 等这个群的第 `turn` 轮完：交回最后一个工具结果（字，状态）。
pub async fn ended(home: &Home, turn: usize) -> (String, String) {
    let events = until_events(&home.root, &venue(), |events| {
        of_kind(events, "turn.ended").len() >= turn
    })
    .await;
    let results = of_kind(&events, "tool.result");
    let last = results.last().expect("调过工具");
    (
        last["body"]["blocks"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .trim_end()
            .to_string(),
        last["body"]["status"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
    )
}

/// 出厂的那一句 `tool-results/<name>.txt`，字段照 `fields` 换，去掉行尾。
pub fn sentence(name: &str, fields: &[(&str, &str)]) -> String {
    let path = resources().join(format!("software/onebot/tool-results/{name}.txt"));
    let mut text = std::fs::read_to_string(path).expect("读得出");
    for (key, value) in fields {
        text = text.replace(&format!("{{{key}}}"), value);
    }
    text.trim_end().to_string()
}

/// 不做的：那一句、状态是 `error`。
pub fn refused(name: &str, fields: &[(&str, &str)]) -> (String, String) {
    (sentence(name, fields), "error".to_string())
}

/// 成了的：那一句、状态是 `ok`。
pub fn done(name: &str, fields: &[(&str, &str)]) -> (String, String) {
    (sentence(name, fields), "ok".to_string())
}

/// 下一个平台动作（撤回以外的：禁言、戳一戳），她发的话跳过。
pub async fn next_platform(napcat: &mut Answering) -> Value {
    loop {
        let action = napcat.action().await;
        if PLATFORM.contains(&action["action"].as_str().unwrap_or_default()) {
            return action;
        }
    }
}

/// 到这时还没取的动作里没有平台动作（撤回另看 `pending_recall`）。
pub fn no_platform(napcat: &mut Answering) {
    while let Some(action) = napcat.pending() {
        let name = action["action"].as_str().unwrap_or_default();
        assert!(!PLATFORM.contains(&name), "不该调：{action}");
    }
    assert!(napcat.pending_recall().is_none(), "不该撤");
}

/// 一次请求的工具面里有哪几件桥的平台工具。
pub fn platform_tools(request: &Request) -> Vec<String> {
    request
        .tools
        .iter()
        .map(|tool| tool.name.clone())
        .filter(|name| ["recall", "mute", "poke"].contains(&name.as_str()))
        .collect()
}

/// 等到终端管理员的私聊会话（管理员名下）里有 `n` 条 `turn.ended`。
pub async fn until_private_turns(home: &Home, n: usize) {
    let deadline = tokio::time::Instant::now() + WAIT;
    while !home
        .sessions()
        .iter()
        .any(|session| of_kind(&home.events(session), "turn.ended").len() >= n)
    {
        assert!(tokio::time::Instant::now() < deadline, "等不到第 {n} 轮完");
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}
