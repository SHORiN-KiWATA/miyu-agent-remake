//! `miyu ask` 的对话（`docs/construction/3-9-miyu-ask（下）.md`）：在进程里起一个核心，在真的套接字上走一遍：
//! 新开一次性会话、边收边打、用量一行；`--continue`、`--session`；没有模型；`--model`（施工 8-10）；Ctrl+C。

use std::sync::Arc;

use serde_json::Value;
use tokio::sync::mpsc;

use crate::support::{AccountIdOf, Asked, Home, plan, within};
use miyu_cli::{Format, Plan, Target};
use miyu_kernel::event::Body;
use miyu_session::testkit::{Play, Script};
use miyu_store::log::first_event;

#[tokio::test]
async fn a_new_oneshot_session_answers_and_shows_the_usage() {
    let home = Home::new(Arc::new(Script::new([Play::Thinks {
        thinking: "想一想",
        text: "你好。",
    }])));
    let Asked {
        code,
        out,
        err,
        screen,
    } = home.ask(&plan("在吗")).await;
    assert_eq!(code, 0, "{err}");
    assert_eq!(
        screen, "想一想\n\n你好。\n· 输入 100 · 命中缓存 40（40%）· 输出 10\n",
        "终端里看到的：思考，空一行，回答，用量"
    );
    assert_eq!(out, "你好。\n");
    assert_eq!(err, "想一想\n\n· 输入 100 · 命中缓存 40（40%）· 输出 10\n");
    let sessions = home.sessions();
    assert_eq!(sessions.len(), 1);
    let created =
        first_event(&home.root.session_dir(&AccountIdOf::admin(), &sessions[0])).expect("读得到");
    assert!(
        matches!(created.body, Body::SessionCreated(created) if created.oneshot),
        "miyu ask 开的是一次性的"
    );
}

#[tokio::test]
async fn continue_goes_on_in_the_last_oneshot_session() {
    let home = Home::new(Arc::new(Script::new([
        Play::Says("一。"),
        Play::Says("二。"),
    ])));
    let Asked { code, err, .. } = home.ask(&plan("第一句")).await;
    assert_eq!(code, 0, "{err}");
    let oneshot = home.sessions()[0].clone();
    // 更新的一个普通会话：`--continue` 要跳过它。
    let plain = home.create_plain().await;
    assert_eq!(home.sessions()[0].as_str(), plain, "普通会话更新");
    let second = Plan {
        target: Target::Continue,
        format: Format::Json,
        ..plan("第二句")
    };
    let Asked { code, out, err, .. } = home.ask(&second).await;
    assert_eq!(code, 0, "{err}");
    let printed: Value = serde_json::from_str(out.trim_end()).expect("一行 JSON");
    assert_eq!(home.sessions().len(), 2, "没有另开会话");
    assert_eq!(printed["session"], oneshot.as_str(), "接的是一次性的那一个");
    assert_eq!(printed["turns"][0]["text"], "二。");
    let said = home
        .log(&oneshot)
        .iter()
        .filter(|event| matches!(event.body, Body::MessageUser(_)))
        .count();
    assert_eq!(said, 2, "两句都在一个会话里");
}

#[tokio::test]
async fn continue_with_nothing_to_go_on_says_so() {
    let home = Home::new(Arc::new(Script::new([])));
    let continuing = Plan {
        target: Target::Continue,
        ..plan("接着说")
    };
    let Asked { code, out, err, .. } = home.ask(&continuing).await;
    assert_eq!(code, 1);
    assert_eq!(out, "");
    assert_eq!(err, "还没有 miyu ask 开过的会话\n");
}

#[tokio::test]
async fn an_unknown_session_is_refused_in_the_heads_language() {
    let home = Home::new(Arc::new(Script::new([])));
    let unknown = Plan {
        target: Target::Session("0192f3a0-0000-7000-8000-000000000001".to_string()),
        ..plan("在吗")
    };
    let Asked { code, err, .. } = home.ask(&unknown).await;
    assert_eq!(code, 1);
    assert!(err.contains("会话"), "中文的拒绝：{err}");
}

/// `--model`（施工 8-10）：新开的会话照它造；接着的先换成它（`@池` 照写的记），以后都用它；换不成的（连同以前的挡位名，
/// 施工 8-8 补）照核心的原话说，退出码 1，不发话。
#[tokio::test]
async fn model_makes_a_new_session_with_it_and_switches_a_continued_one() {
    const CONFIG: &str = "[providers.a]\nlocal = false\n\n[providers.b]\nlocal = false\n\n[models]\nchat = \"a/m\"\n\n[pools.small]\nmodels = [\"b/small\"]\n";
    let script = Script::new([Play::Says("一。"), Play::Says("二。")]);
    let home = Home::configured(Arc::new(script), CONFIG);
    let first = Plan {
        model: Some("b/n".to_string()),
        ..plan("第一句")
    };
    let Asked { code, err, .. } = home.ask(&first).await;
    assert_eq!(code, 0, "{err}");
    let session = home.sessions()[0].clone();
    let created =
        first_event(&home.root.session_dir(&AccountIdOf::admin(), &session)).expect("读得到");
    assert!(
        matches!(&created.body, Body::SessionCreated(created) if created.model.as_deref() == Some("b/n")),
        "新开的照它造：{created:?}"
    );
    let switch = |model: &str, text: &str| Plan {
        target: Target::Continue,
        model: Some(model.to_string()),
        ..plan(text)
    };
    let Asked { code, err, .. } = home.ask(&switch("@small", "第二句")).await;
    assert_eq!(code, 0, "{err}");
    let kinds = |home: &Home| -> Vec<String> {
        home.log(&session)
            .iter()
            .filter_map(|event| match &event.body {
                Body::PolicyChanged(changed) => changed.model.clone(),
                Body::MessageUser(_) => Some("说".to_string()),
                _ => None,
            })
            .collect()
    };
    assert_eq!(kinds(&home), ["说", "@small", "说"], "先换，再说");
    for model in ["c/x", "lite"] {
        let Asked { code, err, .. } = home.ask(&switch(model, "第三句")).await;
        assert_eq!(code, 1, "{model}");
        assert!(!err.is_empty(), "照核心的原话说");
    }
    assert_eq!(kinds(&home), ["说", "@small", "说"], "换不成的不发话");
}

/// `--no-memory`（施工 R-3 下）：新开的会话 `session.create` 带 `"memory": "off"`，记进快照；不写的不带，照人格的。
#[tokio::test]
async fn no_memory_makes_an_off_session() {
    let script = Script::new([Play::Says("一。"), Play::Says("二。")]);
    // 记忆要人格才生效（施工 P-4 上）：默认人格设成出厂的软件工程师。
    let home = Home::configured(Arc::new(script), "[persona]\ndefault = \"engineer\"\n");
    let snapshot = |home: &Home, session: &miyu_kernel::id::SessionId| -> String {
        let dir = home.root.session_dir(&AccountIdOf::admin(), session);
        let created = first_event(&dir).expect("读得到");
        let Body::SessionCreated(created) = &created.body else {
            panic!("第 1 条应该是造会话");
        };
        let bytes = miyu_store::blob::Blobs::new(home.root.blobs(&AccountIdOf::admin()))
            .get(&created.policy)
            .expect("快照在 blob 里");
        String::from_utf8(bytes).expect("是字")
    };
    let off = Plan {
        no_memory: true,
        ..plan("第一句")
    };
    let Asked { code, err, .. } = home.ask(&off).await;
    assert_eq!(code, 0, "{err}");
    let Asked { code, err, .. } = home.ask(&plan("第二句")).await;
    assert_eq!(code, 0, "{err}");
    let sessions = home.sessions();
    let scopes: Vec<bool> = sessions
        .iter()
        .map(|session| snapshot(&home, session).contains(r#""memory":"off""#))
        .collect();
    assert_eq!(scopes.iter().filter(|off| **off).count(), 1, "{sessions:?}");
}

#[tokio::test]
async fn without_a_model_it_is_exit_code_5() {
    // 核心照出厂的档案造路由，配置里什么都没写：每次请求都是 `no_model`（施工 8-6）。
    let home = Home::new(miyu_core::models::routes(&crate::support::resources()).expect("造得出"));
    let Asked { code, out, err, .. } = home.ask(&plan("在吗")).await;
    assert_eq!(code, 5, "{err}");
    assert_eq!(out, "");
    assert_eq!(err, "没有可用的模型：还没配。运行 miyu setup。\n");
}

#[tokio::test]
async fn ctrl_c_once_interrupts_the_turn_and_waits_for_it() {
    let home = Home::new(Arc::new(Script::new([Play::Holds])));
    let (press, presses) = mpsc::channel(4);
    let asking = {
        let plan = plan("在吗");
        let home = &home;
        async move { home.ask_with(&plan, presses).await }
    };
    let pressing = async {
        home.until_a_turn_starts().await;
        press.send(()).await.expect("还在等");
    };
    let (Asked { code, err, .. }, ()) = tokio::join!(asking, pressing);
    assert_eq!(code, 3, "{err}");
    assert!(err.ends_with("打断了\n"), "{err}");
    let session = home.sessions()[0].clone();
    let ended = home
        .log(&session)
        .into_iter()
        .find_map(|event| match event.body {
            Body::TurnEnded(ended) => Some(ended.reason),
            _ => None,
        });
    assert_eq!(
        ended,
        Some(miyu_kernel::event::EndReason::Interrupted),
        "等到这一轮收了尾"
    );
}

#[tokio::test]
async fn ctrl_c_twice_leaves_at_once() {
    let home = Home::new(Arc::new(Script::new([Play::Holds])));
    let (press, presses) = mpsc::channel(4);
    let asking = {
        let plan = plan("在吗");
        let home = &home;
        async move { home.ask_with(&plan, presses).await }
    };
    let pressing = async {
        home.until_a_turn_starts().await;
        press.send(()).await.expect("还在等");
        press.send(()).await.expect("还在等");
    };
    let (Asked { code, err, .. }, ()) =
        within("按两下就走", async { tokio::join!(asking, pressing) }).await;
    assert_eq!(code, 3, "{err}");
    assert!(err.ends_with("打断了\n"), "{err}");
}

/// 核心握手时说沙盒用不了：最先说一句原因和怎么修，只说一次（施工 5-4 下）。
#[tokio::test]
async fn an_unusable_sandbox_is_said_before_anything_else() {
    let home = Home::without_sandbox(
        Arc::new(Script::new([Play::Says("好。")])),
        miyu_sandbox::Unusable::HelperFailed,
    );
    let Asked { code, err, .. } = home.ask(&plan("在吗")).await;
    assert_eq!(code, 0, "{err}");
    assert!(
        err.starts_with(
            "· 沙盒用不了（miyu-sandbox 跑不起来：重装一次 Miyu）：执行命令要你确认，miyu ask 里确认不了\n"
        ),
        "{err}"
    );
    assert_eq!(err.matches("沙盒用不了").count(), 1, "{err}");
}

/// 加进来的目录（施工 5-10 上）：造会话、说话都带着，记进这一轮；`--continue` 时不写 `--add-dir` 的，这一轮就没有。
#[tokio::test]
async fn added_dirs_stay_with_the_session() {
    // 施工 9-7 上起加进来的目录是会话的属性：接着说不写 `--add-dir` 的照旧，写了的整份换掉。
    let home = Home::new(Arc::new(Script::new([
        Play::Says("一。"),
        Play::Says("二。"),
        Play::Says("三。"),
    ])));
    let with = Plan {
        dirs: vec!["/elsewhere".to_string()],
        ..plan("第一句")
    };
    let Asked { code, err, .. } = home.ask(&with).await;
    assert_eq!(code, 0, "{err}");
    let session = home.sessions()[0].clone();
    let again = Plan {
        target: Target::Continue,
        ..plan("第二句")
    };
    let Asked { code, err, .. } = home.ask(&again).await;
    assert_eq!(code, 0, "{err}");
    let other = Plan {
        target: Target::Continue,
        dirs: vec!["/other".to_string()],
        ..plan("第三句")
    };
    let Asked { code, err, .. } = home.ask(&other).await;
    assert_eq!(code, 0, "{err}");
    let dirs: Vec<Vec<String>> = home
        .log(&session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::TurnStarted(started) => Some(started.dirs),
            _ => None,
        })
        .collect();
    assert_eq!(
        dirs,
        [
            vec!["/elsewhere".to_string()],
            vec!["/elsewhere".to_string()],
            vec!["/other".to_string()]
        ]
    );
}

/// 加进来的目录太宽（施工 5-10 上）：造会话时就被拒，照握手时的语言说，退出码 1，不留下一个空的会话。
#[tokio::test]
async fn a_too_wide_added_dir_is_refused_before_a_session_is_made() {
    let home = Home::new(Arc::new(Script::new([])));
    let wide = Plan {
        dirs: vec![home.root.path().to_string_lossy().into_owned()],
        ..plan("在吗")
    };
    let Asked { code, err, .. } = home.ask(&wide).await;
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("加进来的目录太宽"), "中文的拒绝：{err}");
    assert!(home.sessions().is_empty(), "没留下空的会话");
}
