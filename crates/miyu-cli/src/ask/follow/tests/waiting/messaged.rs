//! 这一次以前派、这一次留了言的子代理也等（施工 7-9 补，`docs/blueprint/cli/ask.md`「等子代理」第 1、2、3、6 条）：报回来了
//! 那一行没有标题，两种语言；没留言的以前派的不印不数；那一行的编号、标题写成干净的一行。

use serde_json::json;

use super::*;

#[test]
fn a_subagent_from_before_that_she_messaged_is_waited_for_without_a_title() {
    // 施工 7-9 补：第一轮里她给这一次以前派的 j5 留了言，又派了 j1：两个都等；j5 报回来了那一行没有标题（这边没见过派它的
    // 那一条）。以前派的 j7 没留言，它的回报不印、不数。
    let messaged = json!({"kind": "job.messaged", "job": "j5"});
    for (language, waiting_line, j5, j1) in [
        (
            Language::Chinese,
            "· 等 2 个子代理回报…（按 Ctrl+C 不等了）\n",
            "· j5 报回来了\n",
            "· j1「查 A」报回来了\n",
        ),
        (
            Language::English,
            "· Waiting for 2 subagents to report… (Ctrl+C stops waiting)\n",
            "· j5 reported back\n",
            "· j1 \u{201c}查 A\u{201d} reported back\n",
        ),
    ] {
        let plan = plan(Format::Text, language);
        let mut messages = round(
            10,
            "ask-4",
            "派出去了。",
            12,
            json!([messaged, agent("j1", "查 A")]),
            "completed",
        );
        messages.push(report(20, "j7", "done"));
        messages.push(report(21, "j5", "done"));
        messages.extend(round(
            22,
            "child/report/7",
            "j5 好了。",
            25,
            json!([]),
            "completed",
        ));
        messages.push(report(30, "j1", "done"));
        messages.extend(round(
            31,
            "child/report/9",
            "都好了。",
            36,
            json!([]),
            "completed",
        ));
        let (step, screen, _) = waiting(&plan, false, &messages, |_, _| None);
        assert_eq!(step, Step::Done(exit::OK), "{screen}");
        let expected = format!("派出去了。\n{waiting_line}{j5}\nj5 好了。\n{j1}\n都好了。\n");
        assert!(screen.contains(&expected), "{screen}");
        assert!(!screen.contains("j7"), "{screen}");
    }
}

#[test]
fn the_report_line_is_one_clean_line() {
    // 编号、标题照每一步参数的值写成一行：只取第一行，控制字符换掉（施工 7-9；没有标题的那一行也是，施工 7-9 补）。
    let plan = plan(Format::Text, Language::Chinese);
    let mut messages = round(
        10,
        "ask-4",
        "派出去了。",
        12,
        json!([agent("j1", "查 A\n再查 B"), {"kind": "job.messaged", "job": "j5\u{7}"}]),
        "completed",
    );
    messages.push(report(20, "j1", "done"));
    messages.push(report(21, "j5\u{7}", "done"));
    let (_, screen, _) = waiting(&plan, false, &messages, |_, _| None);
    assert!(
        screen.contains("· j1「查 A…」报回来了\n· j5\u{fffd} 报回来了\n"),
        "{screen}"
    );
}
