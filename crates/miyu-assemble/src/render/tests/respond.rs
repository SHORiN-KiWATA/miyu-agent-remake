//! 照记下的几条开的回合（施工 O-14 上）：那几条旁听的话在回合开始的地方渲染，前面是事实、群聊近况；群会话里一行一条，
//! 别的照原样；近况不收当过触发的；以后的请求一字不差。

use super::*;

const MEMBER: &str = r#"{"kind":"external","venue":"qq:group:1","id":"qq:20017","role":"member"}"#;
const BRIDGE: &str = r#"{"kind":"module","id":"onebot"}"#;

/// 小林旁听的一句。
fn ambient(log: &mut Log, text: &str, msg: &str) -> u64 {
    log.detached(
        MEMBER,
        "message.user",
        &format!(
            r#"{{"blocks":[{}],"venue":{{"msg":"{msg}","name":"小林","ambient":true}}}}"#,
            text_json(text)
        ),
    )
}

/// 桥照 `to` 开一轮，带一块事实。
fn respond(log: &mut Log, to: &[u64]) {
    log.start_on(to);
    log.fact_by(BRIDGE, "judge", "<judge/>");
}

fn in_group(log: &Log) -> Vec<String> {
    shape(&render(log.history(), &group_texts(480)))
}

#[test]
fn the_overheard_triggers_come_where_the_turn_starts() {
    let mut log = Log::new();
    let first = ambient(&mut log, "在吗", "8801");
    ambient(&mut log, "今天好热", "8802");
    let third = ambient(&mut log, "@Miyu 你说呢", "8803");
    respond(&mut log, &[first, third]);
    assert_eq!(
        in_group(&log),
        [concat!(
            "user: <judge/> | <recent>\n[15:00] 小林 [msg=8802]: 今天好热\n",
            " | [15:00] 小林 [msg=8801]: 在吗 | [15:00] 小林 [msg=8803]: @Miyu 你说呢",
        )]
    );
    assert_eq!(
        rendered(&log),
        ["user: <judge/> | 在吗 | @Miyu 你说呢"],
        "不是群会话的照原样"
    );
}

#[test]
fn later_blocks_skip_what_already_opened_a_turn_and_earlier_requests_stay() {
    let mut log = Log::new();
    let first = ambient(&mut log, "在吗", "8801");
    respond(&mut log, &[first]);
    let before = in_group(&log);
    // 回合中途旁听到的两条，后来一条也被桥交了。
    ambient(&mut log, "她在", "8802");
    let late = ambient(&mut log, "那问你个事", "8803");
    log.reply(&format!("[{}]", text_json("在")));
    log.end("completed");
    ambient(&mut log, "嗯", "8804");
    respond(&mut log, &[late]);
    let after = in_group(&log);
    assert_eq!(after[..before.len()], before[..]);
    assert_eq!(
        after.last().map(String::as_str),
        Some(concat!(
            "user: <judge/> | <recent>\n[15:00] 小林 [msg=8802]: 她在\n[15:00] 小林 [msg=8804]: 嗯\n",
            " | [15:00] 小林 [msg=8803]: 那问你个事",
        ))
    );
}
