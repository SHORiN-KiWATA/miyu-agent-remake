//! 在一条连上了的连接上把一句话说完（施工 3-9 下）：握手、传附件（施工 3-9 三补）、找会话、订阅、发，跟着那一轮边收
//! 边打。跟着那一轮的那一段（[`follow_turn`]）`miyu compact` 也用（施工 6-8）。

use serde_json::{Value, json};
use tokio::sync::mpsc;

use miyu_ipc::Connection;

use super::follow::{Follow, Step};
use super::{Plan, Screen, Target, exit};
use crate::language::Language;
use crate::link;
use crate::rpc::Rpc;
use crate::shown::say;

/// 握手、找会话、订阅、发，跟着那一轮边收边打，交回退出码。`presses` 是一次次的 Ctrl+C：第一次打断这一轮
/// （排着的退回），等它收尾；第二次不等了。
pub async fn talk(
    connection: Connection,
    token: &str,
    plan: &Plan,
    screen: &mut Screen<'_>,
    presses: mpsc::Receiver<()>,
) -> u8 {
    let mut rpc = Rpc::new(connection, "ask");
    // 一律说没人能确认：`miyu ask` 里没有确认的界面（`22-命令行.md` O3，2026-09-28 项目主人改），要问人的当场
    // 拒绝、告诉她原因，不一直等着。
    let unsandboxed = match link::hello(&mut rpc, token, &plan.language, false, screen.err).await {
        Ok(hello) => link::unsandboxed(&hello),
        Err(code) => return code,
    };
    // 附件在造会话之前传：传不上的不发话，也不留下一个空的会话（施工 3-9 三补）。
    let attachments = match attach(&mut rpc, plan, screen).await {
        Ok(attachments) => attachments,
        Err(code) => return code,
    };
    let (session, used) = match session(&mut rpc, plan, screen).await {
        Ok(found) => found,
        Err(code) => return code,
    };
    let subscribe = json!({"session": session, "stream": "events"});
    let subscribed = link::request(
        &mut rpc,
        "subscribe",
        subscribe.clone(),
        &plan.language,
        screen.err,
    )
    .await;
    if let Err(code) = subscribed {
        return code;
    }
    let mut send =
        json!({"session": session, "text": plan.text, "cwd": plan.cwd, "dirs": plan.dirs});
    if !attachments.is_empty() {
        send["attachments"] = Value::Array(attachments);
    }
    let sent = match rpc.send("session.send", send).await {
        Ok(sent) => sent,
        Err(error) => {
            say(screen.err, &error.to_string());
            return exit::ERROR;
        }
    };
    let mut follow = Follow::new(&session, &sent, plan);
    // 握手的回应说沙盒用不了：执行命令都要确认，这里确认不了，最先说一句（施工 5-4 下）。
    if let Some(reason) = &unsandboxed {
        follow.unsandboxed(reason, screen);
    }
    // 造会话的回应里说了会话实际在哪个目录里干活：目录太宽的，第一步之前说一句（施工 4-5 下）。
    if let Some(used) = &used {
        follow.moved(used, screen);
    }
    let watching = Watching {
        subscribe: &subscribe,
        session: &session,
        queued: "return",
        language: &plan.language,
    };
    follow_turn(&mut rpc, &mut follow, &watching, screen, presses).await
}

/// 跟着一轮时要的几样：重新订阅用的参数、哪个会话、打断时排着的怎么办（`return` 退回、`send` 接着发）、界面语言。
pub(crate) struct Watching<'a> {
    pub(crate) subscribe: &'a Value,
    pub(crate) session: &'a str,
    pub(crate) queued: &'a str,
    pub(crate) language: &'a Language,
}

/// 跟着那一轮边收边打，交回退出码（施工 6-8 从 [`talk`] 拆出来，`miyu compact` 也用）：收到 `resync` 重新订阅，不补看
/// 掉的那些；第一次 Ctrl+C 打断这一轮，排着的照 `watching.queued` 办，等它收尾；第二次不等了，说「打断了」。
pub(crate) async fn follow_turn(
    rpc: &mut Rpc,
    follow: &mut Follow<'_>,
    watching: &Watching<'_>,
    screen: &mut Screen<'_>,
    mut presses: mpsc::Receiver<()>,
) -> u8 {
    let Watching {
        subscribe,
        session,
        queued,
        language,
    } = watching;
    let mut interrupting = false;
    loop {
        tokio::select! {
            message = rpc.next() => {
                let Some(message) = message else {
                    say(screen.err, &language.disconnected());
                    return exit::ERROR;
                };
                match follow.take(&message, screen) {
                    Step::Going => {}
                    Step::Done(code) => return code,
                    Step::Resubscribe => {
                        if rpc.send("subscribe", (*subscribe).clone()).await.is_err() {
                            say(screen.err, &language.disconnected());
                            return exit::ERROR;
                        }
                    }
                }
            }
            Some(()) = presses.recv() => {
                if interrupting {
                    say(screen.err, &language.interrupted());
                    return exit::INTERRUPTED;
                }
                interrupting = true;
                let interrupt = json!({"session": session, "queued": queued});
                if rpc.send("session.interrupt", interrupt).await.is_err() {
                    say(screen.err, &language.disconnected());
                    return exit::ERROR;
                }
            }
        }
    }
}

/// 照先后把 `--file` 的每一个传给核心（`blob.put`，传路径），交回回应：说话时照原样带着。传不上的，说是哪个文件、核心
/// 说的原因，交回退出码（施工 3-9 三补）。
async fn attach(rpc: &mut Rpc, plan: &Plan, screen: &mut Screen<'_>) -> Result<Vec<Value>, u8> {
    let mut attached = Vec::with_capacity(plan.files.len());
    for file in &plan.files {
        let params = json!({ "path": file });
        let put = link::request_saying(
            rpc,
            "blob.put",
            params,
            &plan.language,
            screen.err,
            |reason| plan.language.not_attached(file, reason),
        );
        attached.push(put.await?);
    }
    Ok(attached)
}

/// 接哪个会话：新开一个一次性的；上一次 `miyu ask` 开的；指定的。交回会话的编号；新开的，再交回核心说的它
/// 实际在哪个目录里干活。
async fn session(
    rpc: &mut Rpc,
    plan: &Plan,
    screen: &mut Screen<'_>,
) -> Result<(String, Option<String>), u8> {
    match &plan.target {
        Target::New => {
            let params = json!({"cwd": plan.cwd, "dirs": plan.dirs, "oneshot": true});
            let result =
                link::request(rpc, "session.create", params, &plan.language, screen.err).await?;
            let session = result["session"].as_str().unwrap_or_default().to_string();
            Ok((session, result["cwd"].as_str().map(str::to_string)))
        }
        Target::Continue => link::latest_oneshot(rpc, &plan.language, screen.err)
            .await
            .map(|session| (session, None)),
        Target::Session(session) => Ok((session.clone(), None)),
    }
}
