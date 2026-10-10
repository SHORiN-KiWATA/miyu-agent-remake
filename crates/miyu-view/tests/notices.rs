//! 旁白：手动压缩、后台任务了结、一组题答了（施工 9-8 上）；翻页时派在更早一页的任务（9-8 中）。

use std::sync::Arc;

use miyu_kernel::event::{Choice, JobReason, Question, Response};
use miyu_kernel::origin::By;
use miyu_kernel::testkit::{Line, Play};
use miyu_view::Projector;

use crate::support::*;

#[test]
fn a_manual_compaction_is_one_notice_without_an_end() {
    let mut stage = stage();
    stage.limits(Some(200_000), Some(8_000));
    stage.model([Line::says("one")]);
    stage.say("first");
    summarizes(&mut stage, "summary of the talk");
    stage.request_compaction(Some("keep the names"));
    let entries = same(&stage);
    let notice = json(entries.last().expect("有条目"));
    assert_eq!(notice["what"], "compaction", "{entries:#?}");
    assert_eq!(notice["state"], "done");
    assert_eq!(notice["trigger"], "manual");
    assert_eq!(notice["instructions"], "keep the names");
    assert!(
        notice["took_ms"].is_u64(),
        "那一轮的用时接在这一条上：{notice}"
    );
    assert_eq!(of_kind(&entries, "end").len(), 1, "压缩那一轮不另起收尾");
}

#[test]
fn a_background_command_reports_with_its_title_and_command() {
    let mut stage = stage();
    stage.model([
        Line::calls(
            "",
            &[(
                "shell",
                r#"{"command":"cargo build","description":"Build","run_in_background":true}"#,
            )],
        ),
        Line::says("Started."),
        Line::says("Build finished."),
    ]);
    stage.tools([Play::starts_command(1, "Build")]);
    stage.say("build");
    stage.job_ends(1, JobReason::Exited, By::Kernel, None);
    let entries = same(&stage);
    let tool = json(of_kind(&entries, "tool")[0]);
    assert_eq!(tool["job"], "j1");
    let notice = json(
        entries
            .iter()
            .rev()
            .find(|e| json(e)["what"] == "job")
            .expect("有任务的旁白"),
    );
    assert_eq!(notice["title"], "Build");
    assert_eq!(notice["job_kind"], "command");
    assert_eq!(notice["mark"], "done");
    assert_eq!(notice["command"], "cargo build");
    assert_eq!(notice["output"]["chars"], 48_213);
}

/// 翻页时任务派在更早的一页（施工 9-8 中）：先照切点前的日志学任务，这一页的回报照样有标题和命令。
#[test]
fn a_page_cut_after_the_start_still_names_the_job() {
    let mut stage = stage();
    stage.model([
        Line::calls(
            "",
            &[(
                "shell",
                r#"{"command":"cargo build","description":"Build","run_in_background":true}"#,
            )],
        ),
        Line::says("Started."),
        Line::says("Build finished."),
    ]);
    stage.tools([Play::starts_command(1, "Build")]);
    stage.say("build");
    stage.job_ends(1, JobReason::Exited, By::Kernel, None);
    let log = stage.log();
    let cut = log
        .iter()
        .position(|event| matches!(event.body, miyu_kernel::event::Body::JobReported(_)))
        .expect("有回报");
    let mut projector = Projector::new(Arc::new(texts("en")), None);
    projector.learn(&log[..cut]);
    for event in &log[cut..] {
        projector.event(event);
    }
    let entries = projector.entries();
    assert!(
        of_kind(entries, "tool").is_empty(),
        "派它的那一步在更早的一页"
    );
    let notice = json(
        entries
            .iter()
            .find(|e| json(e)["what"] == "job")
            .expect("有任务的旁白"),
    );
    assert_eq!(notice["title"], "Build");
    assert_eq!(notice["job_kind"], "command");
    assert_eq!(notice["command"], "cargo build");
}

#[test]
fn answered_questions_carry_what_was_asked() {
    let mut stage = stage();
    let question = Question {
        header: None,
        question: "Which one?".to_string(),
        options: vec![
            Choice {
                label: "A".to_string(),
                description: None,
                preview: None,
            },
            Choice {
                label: "B".to_string(),
                description: None,
                preview: None,
            },
        ],
        multiple: false,
    };
    stage.model([
        Line::calls("", &[("ask_user", r#"{"questions":[]}"#)]),
        Line::says("Thanks."),
    ]);
    stage.tools([Play::Asks(vec![question])]);
    stage.say("ask me");
    let call = stage.ran()[0].0;
    stage.reply(
        call,
        vec![Response {
            picked: vec!["A".to_string()],
            text: None,
            notes: None,
        }],
    );
    let entries = same(&stage);
    let notice = json(
        entries
            .iter()
            .find(|e| json(e)["what"] == "answered")
            .expect("有答了的旁白"),
    );
    assert_eq!(notice["questions"][0]["question"], "Which one?");
    assert_eq!(notice["answers"][0]["picked"][0], "A");
}
