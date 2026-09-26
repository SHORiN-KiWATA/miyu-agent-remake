//! 试玩台接在本机的假 DeepSeek 上跑（`docs/construction/3-5-试玩台（补）.md` 验收第 2 条）：连说两轮，
//! 故意掐断一次。查终端上写了什么、发出去的请求、会话日志。

mod support;

use serde_json::Value;
use support::{Fake, Piece, bench, finish, log_lines, opening, scratch, text, thinking};
use tokio::sync::mpsc;

use miyu_try::{Said, Show};

/// 照说好的几行跑一遍：交回终端上写的、会话的目录。
async fn run(fake: &Fake, root: &std::path::Path, lines: &[&str]) -> (String, std::path::PathBuf) {
    let (tell, said) = mpsc::unbounded_channel();
    for line in lines {
        tell.send(Said::Line((*line).to_string()))
            .expect("试玩台还在听");
    }
    tell.send(Said::End).expect("试玩台还在听");
    let mut out = Vec::new();
    let dir = miyu_try::run(
        bench(&fake.url, root),
        said,
        Show::new(&mut out, false),
        false,
    )
    .await
    .expect("试玩台跑得完");
    (String::from_utf8(out).expect("终端上写的是 UTF-8"), dir)
}

/// 日志里每一条的种类，照先后。
fn kinds(log: &[Value]) -> Vec<String> {
    log.iter()
        .map(|event| event["kind"].as_str().expect("每一条都有种类").to_string())
        .collect()
}

#[tokio::test]
async fn two_turns_stream_log_and_extend_the_prefix() {
    let mut first = vec![opening(), thinking("想一想"), text("你好"), text("呀。")];
    first.extend(finish(20, 0, 5));
    let mut second = vec![opening(), thinking("再想想"), text("好的。")];
    second.extend(finish(40, 20, 3));
    let fake = Fake::start(vec![first, second]).await;
    let root = scratch("two-turns");
    let (shown, dir) = run(&fake, &root, &["你好", "再说一句"]).await;

    // 终端上：说的话、思考、回复、每次请求的用量；第二次写上一次发过的重算了多少。
    for expected in [
        "你> 你好",
        "思考：想一想",
        "你好呀。",
        "（输入 20：命中 0，没命中 20 · 输出 5",
        "你> 再说一句",
        "思考：再想想",
        "好的。",
        "（输入 40：命中 20，没命中 20 · 输出 3",
        "上一次发过的 20 里重算了 0）",
    ] {
        assert!(
            shown.contains(expected),
            "终端上没有「{expected}」：\n{shown}"
        );
    }

    // 发出去的：第二次是第一次的前缀延伸，多了回复和第二句话；key 不在请求体里。
    let bodies = fake.bodies();
    assert_eq!(bodies.len(), 2);
    let then = bodies[0]["messages"].as_array().unwrap();
    let now = bodies[1]["messages"].as_array().unwrap();
    assert_eq!(
        &now[..then.len()],
        &then[..],
        "第二次请求不是第一次的前缀延伸"
    );
    assert_eq!(now.len(), then.len() + 2);
    assert_eq!(now[then.len()]["content"], "你好呀。");
    assert_eq!(now[then.len()]["reasoning_content"], "想一想");
    assert_eq!(bodies[0]["model"], "deepseek-flash");
    for body in &bodies {
        assert!(!body.to_string().contains("test-key"));
    }

    // 日志：两轮都齐了，一次请求一条 model.called，用量记在里面。
    let log = log_lines(&dir);
    let kinds = kinds(&log);
    assert_eq!(kinds[0], "session.created");
    for (kind, count) in [
        ("message.user", 2),
        ("turn.started", 2),
        ("message.assistant", 2),
        ("model.called", 2),
        ("turn.ended", 2),
    ] {
        let found = kinds.iter().filter(|seen| *seen == kind).count();
        assert_eq!(found, count, "日志里 {kind} 有 {found} 条：{kinds:?}");
    }
    let called: Vec<&Value> = log
        .iter()
        .filter(|event| event["kind"] == "model.called")
        .collect();
    assert_eq!(called[1]["body"]["usage"]["cache_read"], 20);
    assert!(dir.starts_with(&root));
    std::fs::remove_dir_all(&root).unwrap();
}

#[tokio::test]
async fn a_cut_reply_is_asked_again_with_the_half_and_the_notice() {
    // 第一次：回复出了几段就停住，等试玩台照 /cut 掐断。第二次：接着说完。
    let first = vec![opening(), text("一"), text("二"), text("三"), Piece::Stall];
    let mut second = vec![opening(), text("四五六。")];
    second.extend(finish(30, 0, 4));
    let fake = Fake::start(vec![first, second]).await;
    let root = scratch("cut");
    let (shown, dir) = run(&fake, &root, &["/cut text 2", "数到六"]).await;

    for expected in [
        "（下一次请求在回复的第 2 段掐断）",
        "（这次请求没成：可以重试的错：cut on purpose by the trial bench (/cut)）",
        "（1.0 秒后第 1/5 次重试）",
        "（半截留下了，跟上一句被打断的提示再请求）",
        "四五六。",
    ] {
        assert!(
            shown.contains(expected),
            "终端上没有「{expected}」：\n{shown}"
        );
    }

    // 第二次请求：半截回复，后面跟着被打断的那一句。
    let bodies = fake.bodies();
    assert_eq!(bodies.len(), 2);
    let messages = bodies[1]["messages"].as_array().unwrap();
    let half = &messages[messages.len() - 2];
    assert_eq!(half["role"], "assistant");
    let half = half["content"].as_str().unwrap();
    assert!(
        !half.is_empty() && "一二三".starts_with(half),
        "半截是 {half:?}"
    );
    let notice = messages[messages.len() - 1]["content"].to_string();
    assert!(notice.contains("<reply-cut>"), "最后一条是 {notice}");

    // 日志：半截带着 interrupted，后面是被打断的事实，这一轮最后正常走完。
    let log = log_lines(&dir);
    let cut = log
        .iter()
        .position(|event| {
            event["kind"] == "context.injected" && event["body"]["kind"] == "reply_cut"
        })
        .unwrap();
    assert_eq!(log[cut - 2]["kind"], "message.assistant");
    assert_eq!(log[cut - 2]["body"]["interrupted"], true);
    assert_eq!(log[cut - 1]["kind"], "model.called");
    assert_eq!(log.last().unwrap()["body"]["reason"], "completed");
    std::fs::remove_dir_all(&root).unwrap();
}
