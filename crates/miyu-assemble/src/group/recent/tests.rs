//! 群聊近况（施工 O-13 下）：收两次触发之间的旁听、不收睡着的；回合中途到的归下一块；排在事实后面、触发前面；别的线的
//! `[you]` 行，自己这条线的不收；撤回的标记，触发以后才撤的不标；预算装不下的从老的去掉、写缺口提示；没有的不出；私聊、
//! 没有近况的字的不出；回报开的回合不算界；以后的请求里那一块一字不差。日志的时刻都是 `2026-09-25T07:00Z`，东八区是 15:00。

use crate::render::render;
use crate::test_support::*;

const MEMBER: &str = r#"{"kind":"external","venue":"qq:group:1","id":"qq:20017","role":"member"}"#;
const BRIDGE: &str = r#"{"kind":"module","id":"onebot"}"#;
const OWN: &str = "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91";
const BRANCH: &str = "0192f3a0-1111-7abc-8def-001122334455";

/// 小林旁听的一句：平台的格另加 `fields`（JSON 对象里的几格，前面带逗号）。不带回合编号。
fn ambient(log: &mut Log, text: &str, msg: &str, fields: &str) -> u64 {
    log.detached(
        MEMBER,
        "message.user",
        &format!(
            r#"{{"blocks":[{}],"venue":{{"msg":"{msg}","name":"小林","ambient":true{fields}}}}}"#,
            text_json(text)
        ),
    )
}

/// 小林说一句，开一轮。
fn opens(log: &mut Log, text: &str, msg: &str) -> u64 {
    let said = log.push(
        MEMBER,
        "message.user",
        &format!(
            r#"{{"blocks":[{}],"venue":{{"msg":"{msg}","name":"小林"}}}}"#,
            text_json(text)
        ),
    );
    log.start(said);
    said
}

/// 她答一句，这一轮结束。
fn answered(log: &mut Log, words: &str) {
    log.reply(&format!("[{}]", text_json(words)));
    log.end("completed");
}

/// 东八区的群会话渲染出来的样子。
fn in_group(log: &Log) -> Vec<String> {
    shape(&render(log.history(), &group_texts(480)))
}

#[test]
fn the_block_holds_what_was_overheard_between_two_triggers() {
    let mut log = Log::new();
    ambient(&mut log, "早", "8801", "");
    ambient(&mut log, "梦话", "8802", r#","asleep":true"#);
    opens(&mut log, "在吗", "8803");
    // 回合进行中到的旁听归下一块；排进这一轮的不算旁听，不进近况。
    ambient(&mut log, "她来了", "8804", "");
    log.push(
        MEMBER,
        "message.user",
        &format!(
            r#"{{"blocks":[{}],"venue":{{"msg":"8806","name":"小林"}}}}"#,
            text_json("等等")
        ),
    );
    answered(&mut log, "在");
    opens(&mut log, "今天谁值班", "8805");
    assert_eq!(
        in_group(&log),
        [
            "user: <recent>\n[15:00] 小林 [msg=8801]: 早\n | [15:00] 小林 [msg=8803]: 在吗 | [15:00] 小林 [msg=8806]: 等等",
            "assistant: 在",
            "user: <recent>\n[15:00] 小林 [msg=8804]: 她来了\n | [15:00] 小林 [msg=8805]: 今天谁值班",
        ]
    );
}

#[test]
fn facts_come_before_the_block_and_the_trigger_comes_last() {
    let mut log = Log::new();
    ambient(&mut log, "早", "8801", "");
    opens(&mut log, "在吗", "8802");
    log.fact("<env/>");
    assert_eq!(
        in_group(&log),
        ["user: <env/> | <recent>\n[15:00] 小林 [msg=8801]: 早\n | [15:00] 小林 [msg=8802]: 在吗"]
    );
}

#[test]
fn what_other_lines_sent_is_a_you_line_and_her_own_line_is_not_repeated() {
    let mut log = Log::new();
    log.owned_by(OWN);
    opens(&mut log, "在吗", "8801");
    answered(&mut log, "在");
    let delivered = |line: &str, msg: &str, text: &str, images: &str| {
        format!(
            r#"{{"line":"{line}","turn":3,"to":["qq:20017"],"msg":"{msg}","text":"{text}"{images}}}"#
        )
    };
    log.detached(BRIDGE, "venue.delivered", &delivered(OWN, "8802", "在", ""));
    log.detached(
        BRIDGE,
        "venue.delivered",
        &delivered(
            BRANCH,
            "8803",
            " 我来<b> ",
            r#","images":["sha256:5f70bf18a086007016e948b04aed3b82103a36bea41755b6cddfaf10ace3c6ef"]"#,
        ),
    );
    log.detached(
        BRIDGE,
        "venue.delivered",
        &delivered(BRANCH, "8804", "", ""),
    );
    opens(&mut log, "好", "8805");
    assert_eq!(
        in_group(&log).last().map(String::as_str),
        Some(
            "user: <recent>\n[15:00] [you] [msg=8803]: 我来\\u003cb\\u003e [image]\n[15:00] [you] [msg=8804]: <no-text>\n | [15:00] 小林 [msg=8805]: 好"
        )
    );
}

#[test]
fn recalls_before_the_trigger_are_marked_and_later_ones_are_not() {
    let mut log = Log::new();
    ambient(&mut log, "发错了", "8801", r#","show_ids":true"#);
    ambient(&mut log, "广告", "8802", r#","show_ids":true"#);
    ambient(&mut log, "广告", "8803", "");
    ambient(&mut log, "留着", "8804", "");
    for (msg, by) in [
        ("8801", "qq:20017"),
        ("8802", "qq:20018"),
        ("8803", "qq:20018"),
    ] {
        log.detached(
            BRIDGE,
            "venue.recalled",
            &format!(r#"{{"msg":"{msg}","by":"{by}"}}"#),
        );
    }
    opens(&mut log, "嗯", "8805");
    let before = in_group(&log);
    log.detached(
        BRIDGE,
        "venue.recalled",
        r#"{"msg":"8804","by":"qq:20017"}"#,
    );
    assert_eq!(
        before,
        [concat!(
            "user: <recent>\n",
            "[15:00] 小林 (id=qq:20017) [msg=8801] (recalled): 发错了\n",
            "[15:00] 小林 (id=qq:20017) [msg=8802] (recalled by qq:20018): 广告\n",
            "[15:00] 小林 [msg=8803] (recalled): 广告\n",
            "[15:00] 小林 [msg=8804]: 留着\n",
            " | [15:00] 小林 [msg=8805]: 嗯",
        )]
    );
    assert_eq!(in_group(&log), before, "触发以后才撤的不改");
}

#[test]
fn the_budget_keeps_the_newest_and_says_how_many_did_not_fit() {
    let mut log = Log::new();
    // 每一行 34 字节，连换行 35：预算 70 正好装两行。
    for msg in ["1", "2", "3"] {
        ambient(&mut log, "aaaaaaaaaa", msg, "");
    }
    opens(&mut log, "嗯", "4");
    assert_eq!(
        shape(&render(log.history(), &group_texts_within(480, 70))),
        [concat!(
            "user: <recent>\n<omitted 1>\n",
            "[15:00] 小林 [msg=2]: aaaaaaaaaa\n",
            "[15:00] 小林 [msg=3]: aaaaaaaaaa\n",
            " | [15:00] 小林 [msg=4]: 嗯",
        )]
    );
    assert_eq!(
        shape(&render(log.history(), &group_texts_within(480, 105))),
        [concat!(
            "user: <recent>\n",
            "[15:00] 小林 [msg=1]: aaaaaaaaaa\n",
            "[15:00] 小林 [msg=2]: aaaaaaaaaa\n",
            "[15:00] 小林 [msg=3]: aaaaaaaaaa\n",
            " | [15:00] 小林 [msg=4]: 嗯",
        )],
        "正好装下的不写缺口提示"
    );
}

#[test]
fn nothing_overheard_private_chats_and_older_snapshots_have_no_block() {
    let mut log = Log::new();
    opens(&mut log, "在吗", "8801");
    assert_eq!(in_group(&log), ["user: [15:00] 小林 [msg=8801]: 在吗"]);
    let mut log = Log::new();
    ambient(&mut log, "早", "8801", "");
    opens(&mut log, "在吗", "8802");
    assert_eq!(shape(&render(log.history(), &texts())), ["user: 在吗"]);
    let mut older = group_texts(480);
    if let Some(chat) = older.group.as_mut() {
        chat.recent = None;
    }
    assert_eq!(
        shape(&render(log.history(), &older)),
        ["user: [15:00] 小林 [msg=8802]: 在吗"]
    );
}

#[test]
fn a_turn_opened_by_a_report_is_not_a_boundary() {
    let mut log = Log::new();
    opens(&mut log, "跑一下测试", "8800");
    let call = log.reply_calling("派出去。");
    let effects = r#"[{"kind":"job.started","job":"j1","what":"command","title":"跑测试"}]"#;
    log.push(
        KERNEL,
        "tool.result",
        &format!(r#"{{"call_id":"{call}","status":"ok","blocks":[],"effects":{effects}}}"#),
    );
    answered(&mut log, "派出去了。");
    ambient(&mut log, "早", "8801", "");
    let reported = log.detached(
        KERNEL,
        "job.reported",
        r#"{"job":"j1","reason":"exited","exit_code":0}"#,
    );
    log.start(reported);
    answered(&mut log, "跑完了");
    ambient(&mut log, "她在", "8802", "");
    opens(&mut log, "在吗", "8803");
    assert_eq!(
        in_group(&log).last().map(String::as_str),
        Some(
            "user: <recent>\n[15:00] 小林 [msg=8801]: 早\n[15:00] 小林 [msg=8802]: 她在\n | [15:00] 小林 [msg=8803]: 在吗"
        )
    );
}

#[test]
fn later_requests_render_the_same_block() {
    let mut log = Log::new();
    ambient(&mut log, "早", "8801", "");
    opens(&mut log, "在吗", "8802");
    let first = in_group(&log);
    log.detached(
        BRIDGE,
        "venue.recalled",
        r#"{"msg":"8801","by":"qq:20017"}"#,
    );
    ambient(&mut log, "又来了", "8803", "");
    answered(&mut log, "在");
    opens(&mut log, "嗯", "8804");
    let later = in_group(&log);
    assert_eq!(later[..first.len()], first[..]);
    assert_eq!(
        later.last().map(String::as_str),
        Some("user: <recent>\n[15:00] 小林 [msg=8803]: 又来了\n | [15:00] 小林 [msg=8804]: 嗯")
    );
}

/// 施工 O-33：近况里旁听的一行照群里的一行的写法：不止一样的标第几个，带大小。
#[test]
fn overheard_media_carry_their_number_and_size() {
    let mut log = Log::new();
    log.detached(
        MEMBER,
        "message.user",
        r#"{"blocks":[],"venue":{"msg":"8895","name":"小林","ambient":true,"media":[{"kind":"image","id":"i-1","size":1500},{"kind":"image","id":"i-2","size":999500}]}}"#,
    );
    opens(&mut log, "看看", "8896");
    assert_eq!(
        shape(&render(log.history(), &group_texts_o33(480))),
        [
            "user: <recent>\n[15:00] 小林 [msg=8895]: [image #1: 2 KB] [image #2: 1.0 MB]\n | [15:00] 小林 [msg=8896]: 看看"
        ]
    );
}
