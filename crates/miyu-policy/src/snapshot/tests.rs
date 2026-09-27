//! 快照的字节、读回来、造策略。随核心附带的字用仓库里出厂的那一份（编译时拿进来，不是读文件）。

use super::*;
use crate::compose::{PersonaTexts, Sources, compose};

/// 出厂的随核心附带的字。
fn core() -> CoreTexts {
    CoreTexts {
        checkpoint_open: include_str!("../../../../resources/core/checkpoint-open.txt").to_string(),
        checkpoint_close: include_str!("../../../../resources/core/checkpoint-close.txt")
            .to_string(),
        turn_ended: TurnEndedTexts {
            interrupted: include_str!("../../../../resources/core/turn-ended/interrupted.txt")
                .to_string(),
            error: include_str!("../../../../resources/core/turn-ended/error.txt").to_string(),
            step_limit: include_str!("../../../../resources/core/turn-ended/step_limit.txt")
                .to_string(),
            aborted: include_str!("../../../../resources/core/turn-ended/aborted.txt").to_string(),
            restarted: include_str!("../../../../resources/core/turn-ended/restarted.txt")
                .to_string(),
        },
        facts: FactTexts {
            env: include_str!("../../../../resources/core/facts/env.txt").to_string(),
            permission: include_str!("../../../../resources/core/facts/permission.txt").to_string(),
            reply_cut: include_str!("../../../../resources/core/facts/reply-cut.txt").to_string(),
        },
        tool_results: ToolResultTexts {
            unknown: include_str!("../../../../resources/core/tool-results/unknown.txt")
                .to_string(),
            not_an_object: include_str!(
                "../../../../resources/core/tool-results/not-an-object.txt"
            )
            .to_string(),
            cancelled_before: include_str!(
                "../../../../resources/core/tool-results/cancelled-before.txt"
            )
            .to_string(),
            cancelled_running: include_str!(
                "../../../../resources/core/tool-results/cancelled-running.txt"
            )
            .to_string(),
            skipped: include_str!("../../../../resources/core/tool-results/skipped.txt")
                .to_string(),
            read_only: include_str!("../../../../resources/core/tool-results/read-only.txt")
                .to_string(),
            denied: include_str!("../../../../resources/core/tool-results/denied.txt").to_string(),
            denied_with_reason: include_str!(
                "../../../../resources/core/tool-results/denied-with-reason.txt"
            )
            .to_string(),
            unattended: include_str!("../../../../resources/core/tool-results/unattended.txt")
                .to_string(),
            question_interrupted: include_str!(
                "../../../../resources/core/tool-results/question-interrupted.txt"
            )
            .to_string(),
            question_voided: include_str!(
                "../../../../resources/core/tool-results/question-voided.txt"
            )
            .to_string(),
            question_unattended: include_str!(
                "../../../../resources/core/tool-results/question-unattended.txt"
            )
            .to_string(),
            restarted: include_str!("../../../../resources/core/tool-results/restarted.txt")
                .to_string(),
        },
        drivers: DriverPlaceholders {
            image_omitted: include_str!("../../../../resources/core/drivers/image-omitted.txt")
                .to_string(),
            file_omitted: include_str!("../../../../resources/core/drivers/file-omitted.txt")
                .to_string(),
            no_output: include_str!("../../../../resources/core/drivers/no-output.txt").to_string(),
            tool_attachments: include_str!(
                "../../../../resources/core/drivers/tool-attachments.txt"
            )
            .to_string(),
            tool_attachments_only: include_str!(
                "../../../../resources/core/drivers/tool-attachments-only.txt"
            )
            .to_string(),
        },
    }
}

/// 软件工程师的快照，有人能确认。
fn engineer() -> Snapshot {
    compose(
        "engineer",
        Sources {
            core: core(),
            persona: PersonaTexts {
                persona: include_str!("../../../../resources/personas/engineer/prompts/persona.md")
                    .to_string(),
            },
        },
        true,
    )
}

#[test]
fn the_engineer_system_is_the_one_sentence() {
    let snapshot = engineer();
    assert_eq!(snapshot.persona, "engineer");
    assert_eq!(snapshot.system, "You are a helpful software engineer.");
    assert_eq!(snapshot.step_limit, None);
    assert_eq!(snapshot.resumes, 3);
}

#[test]
fn the_same_sources_give_the_same_bytes_and_they_read_back() {
    let (one, two) = (engineer(), engineer());
    assert_eq!(one.to_bytes(), two.to_bytes());
    assert_eq!(one.hash(), two.hash());
    assert_eq!(one.hash(), ContentHash::of(&one.to_bytes()));
    assert_eq!(Snapshot::from_bytes(&one.to_bytes()), Ok(one.clone()));
    // 字段的先后就是字节里的先后，紧凑、不换行。
    let text = String::from_utf8(one.to_bytes()).unwrap();
    assert!(text.starts_with(r#"{"persona":"engineer","system":"You are a helpful software engineer.","core":{"checkpoint_open":"#), "{text}");
    assert!(
        text.ends_with(r#""step_limit":null,"attended":true,"resumes":3}"#),
        "{text}"
    );
    // 改一个字，哈希就变了。
    let mut other = one.clone();
    other.system.push('!');
    assert_ne!(other.hash(), one.hash());
}

#[test]
fn broken_bytes_do_not_read_back() {
    let error = Snapshot::from_bytes(b"{\"persona\":1}").unwrap_err();
    assert!(
        error.to_string().starts_with("策略快照读不回来："),
        "{error}"
    );
}

#[test]
fn the_policy_is_built_and_a_broken_template_is_named() {
    let snapshot = engineer();
    let policy = snapshot.policy().unwrap();
    assert!(policy.attended);
    assert_eq!(policy.resumes, 3);
    assert!(policy.tools.is_empty());
    assert!(snapshot.driver_texts().is_ok());
    let mut broken = snapshot.clone();
    broken.core.facts.env = "<env time=\"{time\"/>".to_string();
    let error = broken.policy().err().unwrap();
    assert_eq!(error.which, "事实模板");
    let mut broken = snapshot.clone();
    broken.core.tool_results.unknown = "{nope".to_string();
    assert_eq!(broken.policy().err().unwrap().which, "内核替工具写的几句");
    let mut broken = snapshot;
    broken.core.drivers.file_omitted = "{".to_string();
    let error = broken.driver_texts().unwrap_err();
    assert!(
        error
            .to_string()
            .starts_with("随核心附带的驱动的占位用不了："),
        "{error}"
    );
}

#[test]
fn the_session_is_created_with_the_snapshot_hash() {
    let snapshot = engineer();
    let created = snapshot.session_created(
        AccountId::parse("local").unwrap(),
        VenueId::parse("local").unwrap(),
        Permission {
            level: miyu_kernel::event::Level::Workspace,
            read_only: false,
        },
    );
    assert_eq!(created.policy, snapshot.hash());
}

#[test]
fn the_switches_are_carried_as_given() {
    // 没人能确认的场所：拼的时候照给的记，造策略时照快照的带。
    let sources = Sources {
        core: core(),
        persona: PersonaTexts {
            persona: "x".to_string(),
        },
    };
    let unattended = compose("engineer", sources, false);
    assert!(!unattended.attended);
    assert!(!unattended.policy().unwrap().attended);
    // 步数上限照快照的带：不限的是不限，定了的是那个数。
    assert_eq!(engineer().policy().unwrap().step_limit, None);
    let mut limited = engineer();
    limited.step_limit = Some(5);
    assert_eq!(limited.policy().unwrap().step_limit, Some(5));
}

#[test]
fn each_driver_placeholder_is_its_own() {
    let texts = engineer().driver_texts().unwrap();
    let drivers = core().drivers;
    assert_eq!(texts.image_omitted(), drivers.image_omitted);
    assert_eq!(texts.no_output(), drivers.no_output);
    assert_eq!(texts.tool_attachments(), drivers.tool_attachments);
    assert_eq!(texts.tool_attachments_only(), drivers.tool_attachments_only);
    assert!(
        texts
            .file_omitted("a.pdf", "application/pdf")
            .contains("a.pdf")
    );
}
