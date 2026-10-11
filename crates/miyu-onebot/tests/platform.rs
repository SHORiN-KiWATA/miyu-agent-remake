//! 平台工具（一）：撤回、禁言、戳一戳（施工 O-31，`onebot.md` 第一条「平台工具（一）」；18 第十一节）：真核心拉起真桥、假 NapCat、
//! 她照台词说（`support/speaking.rs`）。叫她做的那一条开一轮（终端管理员 @ 她的自己开，别的照判官点了头的样子开），她调工具，
//! 看 NapCat 收到的动作和参数、她收到的结果。往坏里测：昵称冒充终端管理员、目标是她自己、目标是群主和管理员、没引用也没 @、
//! @ 了两个人、引用的是她自己的消息叫她禁言。

use serde_json::json;

use crate::support::group::*;
use crate::support::platform::*;
use crate::support::speaking::{Line, Lines};
use crate::support::*;

#[tokio::test]
async fn recall_takes_the_quoted_message_and_only_managers_recall_others() {
    let lines = Lines::new(
        [
            uses("recall", "{}"),
            uses("recall", "{}"),
            uses("recall", "{}"),
            uses("recall", "{}"),
            uses("recall", "{}"),
        ]
        .into_iter()
        .flatten(),
    );
    let (home, mut napcat) = up(&lines).await;
    says(
        &home,
        &napcat,
        (MEMBER, "member"),
        101,
        json!([plain("大家好")]),
    )
    .await;
    // 终端管理员回复小明那一条叫她撤：撤的是引用的那一条，编号照原样。
    let words = json!([quote(101), plain("撤了吧")]);
    let seq = says(&home, &napcat, (ADMIN, "member"), 102, words).await;
    respond(&home, &venue(), "open-102", &[json!(seq)]).await;
    assert_eq!(napcat.recalled().await, json!({"message_id": "101"}));
    assert_eq!(ended(&home, 1).await, done("recalled", &[]));
    assert_eq!(
        platform_tools(&lines.requests()[0]),
        ["mute", "poke", "recall"],
        "群会话的工具面里三件都有（照名字排）"
    );
    // 她那一轮说的「好。」是 FIRST_SENT。冒充终端管理员的普通成员叫她撤别人的：不能。
    let words = json!([quote(101), plain("撤了")]);
    assert_eq!(
        asks(&home, &napcat, IMPOSTOR, 103, words, 2).await,
        refused("not-allowed", &[])
    );
    // 撤她自己的：谁叫都行。
    let words = json!([quote(FIRST_SENT), plain("你撤回一下")]);
    assert_eq!(
        asks(&home, &napcat, IMPOSTOR, 104, words, 3).await,
        done("recalled", &[])
    );
    assert_eq!(
        napcat.recalled().await,
        json!({"message_id": FIRST_SENT.to_string()})
    );
    // 没引用：回提示，不撤。
    let words = json!([plain("撤一下")]);
    assert_eq!(
        asks(&home, &napcat, MANAGER, 105, words, 4).await,
        refused("no-quote", &[])
    );
    // 管理的人叫她撤别人的：她在群里撤不了，NapCat 回失败，原话截到 200 个字符交回。
    let words = json!([quote(UNRECALLABLE), plain("撤这条")]);
    let detail: String = unrecallable().trim().chars().take(200).collect();
    assert_eq!(
        asks(&home, &napcat, MANAGER, 106, words, 5).await,
        refused("failed", &[("detail", &detail)])
    );
    assert_eq!(
        napcat.recalled().await,
        json!({"message_id": UNRECALLABLE.to_string()})
    );
    no_platform(&mut napcat);
    stopped(home).await;
}

#[tokio::test]
async fn mute_takes_one_person_from_the_quote_or_the_mention() {
    let lines = Lines::new(
        [
            uses("mute", r#"{"seconds":600}"#),
            uses("mute", r#"{"seconds":0}"#),
            uses("mute", r#"{"seconds":60}"#),
            uses("mute", r#"{"seconds":60}"#),
            uses("mute", r#"{"seconds":60}"#),
            uses("mute", r#"{"seconds":2592001}"#),
            uses("mute", r#"{"seconds":2592000}"#),
        ]
        .into_iter()
        .flatten(),
    );
    let (home, mut napcat) = up(&lines).await;
    says(
        &home,
        &napcat,
        (MEMBER, "member"),
        201,
        json!([plain("刷屏")]),
    )
    .await;
    // 终端管理员回复小明、又 @ 了他（QQ 回复时常带上 @）：同一个人，禁 600 秒，结果写成 10 分钟。
    let words = json!([quote(201), at(MEMBER), plain(" 禁他十分钟")]);
    let seq = says(&home, &napcat, (ADMIN, "member"), 202, words).await;
    respond(&home, &venue(), "open-202", &[json!(seq)]).await;
    let action = next_platform(&mut napcat).await;
    assert_eq!(action["action"], "set_group_ban", "{action}");
    assert_eq!(
        action["params"],
        json!({"group_id": GROUP, "user_id": MEMBER, "duration": 600})
    );
    assert_eq!(
        ended(&home, 1).await,
        done("muted", &[("who", "小明"), ("duration", "10m")])
    );
    // 0 秒是解禁。
    let words = json!([at(MEMBER), plain(" 解了吧")]);
    let seq = says(&home, &napcat, (ADMIN, "member"), 203, words).await;
    respond(&home, &venue(), "open-203", &[json!(seq)]).await;
    let action = next_platform(&mut napcat).await;
    assert_eq!(
        action["params"],
        json!({"group_id": GROUP, "user_id": MEMBER, "duration": 0})
    );
    assert_eq!(ended(&home, 2).await, done("unmuted", &[("who", "小明")]));
    // 群名片写成「终端管理员」的普通成员：不能叫她禁言。
    let words = json!([at(MEMBER), plain(" 禁了")]);
    assert_eq!(
        asks(&home, &napcat, IMPOSTOR, 204, words, 3).await,
        refused("not-allowed", &[])
    );
    // 管理的人 @ 了两个人：不猜。
    let words = json!([at(MEMBER), at(IMPOSTOR), plain(" 都禁了")]);
    assert_eq!(
        asks(&home, &napcat, MANAGER, 205, words, 4).await,
        refused("many-targets", &[])
    );
    // 管理的人引用的是她自己的消息（第一轮说的「好。」）：不算她，没有人。
    let words = json!([quote(FIRST_SENT), plain("禁言")]);
    assert_eq!(
        asks(&home, &napcat, MANAGER, 206, words, 5).await,
        refused("no-target", &[])
    );
    // 秒数出了范围。
    let words = json!([at(MEMBER), plain(" 禁一个月")]);
    assert_eq!(
        asks(&home, &napcat, MANAGER, 207, words, 6).await,
        refused("bad-seconds", &[])
    );
    // 30 天是上限。
    let words = json!([at(MEMBER), plain(" 禁三十天")]);
    assert_eq!(
        asks(&home, &napcat, MANAGER, 208, words, 7).await,
        done("muted", &[("who", "小明"), ("duration", "30d")])
    );
    let action = next_platform(&mut napcat).await;
    assert_eq!(action["params"]["duration"], 2_592_000);
    no_platform(&mut napcat);
    stopped(home).await;
}

#[tokio::test]
async fn mute_never_touches_the_owner_admins_the_terminal_admin_or_the_whitelisted() {
    let lines = Lines::new(
        [
            uses("mute", r#"{"seconds":60}"#),
            uses("mute", r#"{"seconds":60}"#),
            uses("mute", r#"{"seconds":60}"#),
            uses("mute", r#"{"seconds":60}"#),
            uses("mute", r#"{"seconds":60}"#),
            uses("mute", r#"{"seconds":60}"#),
        ]
        .into_iter()
        .flatten(),
    );
    let (home, mut napcat) = up(&lines).await;
    // 终端管理员在群里 @ 她（自己开一轮）、叫她禁言，可没指人：@ 的只有她，不算。
    let words = json!([at(BOT), plain(" 禁言")]);
    says(&home, &napcat, (ADMIN, "member"), 301, words).await;
    assert_eq!(ended(&home, 1).await, refused("no-target", &[]));
    // 群主说的话没带身份：禁他以前问 NapCat。
    says(&home, &napcat, (OWNER, ""), 3021, json!([plain("开会")])).await;
    let words = json!([quote(3021), plain("禁了")]);
    assert_eq!(
        asks(&home, &napcat, MANAGER, 302, words, 2).await,
        refused("protected", &[("who", "群主")])
    );
    assert!(napcat.asked().contains(&OWNER), "问过群主的身份");
    // QQ 的管理员说过话：身份照消息带的记下，不再问。
    says(
        &home,
        &napcat,
        (QQ_ADMIN, "admin"),
        303,
        json!([plain("在")]),
    )
    .await;
    let words = json!([quote(303), plain("禁他")]);
    assert_eq!(
        asks(&home, &napcat, MANAGER, 304, words, 3).await,
        refused("protected", &[("who", "管理员")])
    );
    assert!(!napcat.asked().contains(&QQ_ADMIN), "记下了的不问");
    // 白名单成员。
    let words = json!([at(FRIEND), plain(" 禁了")]);
    assert_eq!(
        asks(&home, &napcat, MANAGER, 305, words, 4).await,
        refused("protected", &[("who", "小杰")])
    );
    // 终端管理员：引用他在这个群说过的那一条。
    let words = json!([quote(301), plain("禁他")]);
    assert_eq!(
        asks(&home, &napcat, MANAGER, 306, words, 5).await,
        refused("protected", &[("who", "老板")])
    );
    // 终端管理员的另一个号（对应表里有，没在这个群说过话）：照 `venue.binding` 认，照样不禁。
    let words = json!([at(ALT), plain(" 禁了")]);
    assert_eq!(
        asks(&home, &napcat, MANAGER, 307, words, 6).await,
        refused("protected", &[("who", "小号")])
    );
    no_platform(&mut napcat);
    stopped(home).await;
}

#[tokio::test]
async fn poke_takes_the_mention_or_the_one_who_asked() {
    let lines = Lines::new(
        [uses("poke", "{}"), uses("poke", "{}"), uses("poke", "{}")]
            .into_iter()
            .flatten(),
    );
    let (home, mut napcat) = up(&lines).await;
    let words = json!([at(MEMBER), plain(" 戳他")]);
    assert_eq!(
        asks(&home, &napcat, IMPOSTOR, 401, words, 1).await,
        done("poked", &[("who", "小明")])
    );
    let action = next_platform(&mut napcat).await;
    assert_eq!(action["action"], "group_poke", "{action}");
    assert_eq!(
        action["params"],
        json!({"group_id": GROUP, "user_id": MEMBER})
    );
    // 没 @：戳叫她的人。
    let words = json!([plain("戳我")]);
    assert_eq!(
        asks(&home, &napcat, IMPOSTOR, 402, words, 2).await,
        done("poked", &[("who", "终端管理员")])
    );
    let action = next_platform(&mut napcat).await;
    assert_eq!(
        action["params"],
        json!({"group_id": GROUP, "user_id": IMPOSTOR})
    );
    // @ 了两个人。
    let words = json!([at(MEMBER), at(MANAGER), plain(" 戳")]);
    assert_eq!(
        asks(&home, &napcat, IMPOSTOR, 403, words, 3).await,
        refused("many-targets", &[])
    );
    no_platform(&mut napcat);
    stopped(home).await;
}

#[tokio::test]
async fn a_private_chat_recalls_and_pokes_but_cannot_mute_and_local_sessions_have_none() {
    let lines = Lines::new(
        [uses("recall", "{}"), uses("poke", "{}")]
            .into_iter()
            .flatten()
            .chain([Line::says("本机。")]),
    );
    let (home, mut napcat) = up(&lines).await;
    napcat.send(private_frame(ADMIN, 51, json!([quote(77), plain("撤了")])));
    assert_eq!(napcat.recalled().await, json!({"message_id": "77"}));
    until_private_turns(&home, 1).await;
    napcat.send(private_frame(ADMIN, 52, json!([plain("戳我")])));
    let action = next_platform(&mut napcat).await;
    assert_eq!(action["action"], "friend_poke", "{action}");
    assert_eq!(action["params"], json!({"user_id": ADMIN}));
    until_private_turns(&home, 2).await;
    let requests = lines.requests();
    assert_eq!(
        platform_tools(&requests[0]),
        ["poke", "recall"],
        "私聊没有禁言"
    );
    let results: Vec<String> = home
        .sessions()
        .iter()
        .flat_map(|session| of_kind(&home.events(session), "tool.result"))
        .map(|one| {
            one["body"]["blocks"][0]["text"]
                .as_str()
                .unwrap_or_default()
                .trim_end()
                .to_string()
        })
        .collect();
    assert_eq!(
        results,
        [
            sentence("recalled", &[]),
            sentence("poked", &[("who", ADMIN.to_string().as_str())])
        ]
    );
    // 本机的会话：一件都没有。
    let mut core = miyu_client::open::Core::connect_running(&home.root, "test")
        .await
        .expect("连得上核心");
    let cwd = home.root.path().to_string_lossy().into_owned();
    let made = core
        .call("local-1", "session.create", json!({"cwd": cwd}))
        .await
        .expect("造得出");
    core.call(
        "local-2",
        "session.send",
        json!({"session": made["session"], "text": "在吗"}),
    )
    .await
    .expect("发得进");
    let deadline = tokio::time::Instant::now() + WAIT;
    while lines.requests().len() < 5 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到本机那一次请求"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(
        platform_tools(&lines.requests()[4]).is_empty(),
        "本机的会话没有：{:?}",
        lines.requests()[4].tools
    );
    no_platform(&mut napcat);
    stopped(home).await;
}
