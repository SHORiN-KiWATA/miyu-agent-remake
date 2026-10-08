//! `miyu rename`（施工 3-8 五补，`docs/blueprint/cli/rename.md`）：在进程里起一个核心，给上一次 `miyu ask` 开的会话起名，
//! 几个词用一个空格连起来，什么都不印，退出码 0，日志里记一条人改的 `session.meta_changed`；`--session` 起名的是指定的那个；
//! 核心不收的（全是空白的）照核心的话说、退出码 1；一个会话都没有的说清楚、退出码 1。

use std::sync::Arc;

use crate::support::{Asked, Home, Tape, plan, within};
use miyu_cli::language::Language;
use miyu_cli::{RenamePlan, rename_on};
use miyu_kernel::event::{Body, MetaChanged};
use miyu_kernel::id::SessionId;
use miyu_kernel::origin::By;
use miyu_session::testkit::{Play, Script};

/// 给 `session`（没有的是上一次 `miyu ask` 开的）起名 `title`，界面是中文。
fn renaming(session: Option<&SessionId>, title: &str) -> RenamePlan {
    RenamePlan {
        session: session.map(|session| session.as_str().to_string()),
        title: title.to_string(),
        language: Language::Chinese,
    }
}

/// 在真的套接字上连上核心，照 `plan` 起一次名。
async fn rename(home: &Home, plan: &RenamePlan) -> Asked {
    let (connection, token) = miyu_ipc::connect(&home.root).await.expect("连得上");
    let tape = Tape::default();
    let mut err = tape.pen(true);
    let code = within("起完名", rename_on(connection, &token, plan, &mut err)).await;
    Asked {
        code,
        out: tape.text(|err| !err),
        err: tape.text(|err| err),
        screen: tape.text(|_| true),
    }
}

/// 会话 `session` 日志里的 `session.meta_changed`：谁改的、改成了什么标题。
fn titles(home: &Home, session: &SessionId) -> Vec<(By, Option<String>)> {
    home.log(session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::MetaChanged(MetaChanged { title, .. }) => Some((event.by, title)),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn the_last_ask_gets_the_title_and_nothing_is_printed() {
    let home = Home::new(Arc::new(Script::new([Play::Says("好。")])));
    assert_eq!(home.ask(&plan("hi")).await.code, 0);
    let renamed = rename(&home, &renaming(None, "修 CI 的红")).await;
    assert_eq!(
        (renamed.code, renamed.screen.as_str()),
        (0, ""),
        "什么都不印"
    );
    let session = home.sessions()[0].clone();
    let [(by, title)] = titles(&home, &session).try_into().expect("记了一条");
    assert!(matches!(by, By::Person(_)), "人改的：{by:?}");
    assert_eq!(title.as_deref(), Some("修 CI 的红"));
}

/// `--session` 起名的是指定的那个会话，不是最新的那个一次性会话。
#[tokio::test]
async fn a_given_session_is_the_one_renamed() {
    let home = Home::new(Arc::new(Script::new([
        Play::Says("一。"),
        Play::Says("二。"),
    ])));
    assert_eq!(home.ask(&plan("说一")).await.code, 0);
    assert_eq!(home.ask(&plan("说二")).await.code, 0);
    let sessions = home.sessions();
    let (latest, first) = (&sessions[0], &sessions[1]);
    let renamed = rename(&home, &renaming(Some(first), "第一个")).await;
    assert_eq!(renamed.code, 0, "{}", renamed.err);
    assert_eq!(titles(&home, first).len(), 1);
    assert!(titles(&home, latest).is_empty(), "最新的那个没动");
}

#[tokio::test]
async fn a_refused_title_or_no_session_says_so() {
    let home = Home::new(Arc::new(Script::new([Play::Says("好。")])));
    let renamed = rename(&home, &renaming(None, "标题")).await;
    assert_eq!(renamed.code, 1);
    assert!(
        renamed.err.contains("还没有 miyu ask 开过的会话"),
        "{}",
        renamed.err
    );
    assert_eq!(home.ask(&plan("hi")).await.code, 0);
    let renamed = rename(&home, &renaming(None, "   ")).await;
    assert_eq!((renamed.code, renamed.out.as_str()), (1, ""));
    assert!(!renamed.err.is_empty(), "照核心的话说");
    assert!(titles(&home, &home.sessions()[0]).is_empty(), "什么都没记");
}
