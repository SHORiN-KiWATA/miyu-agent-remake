//! 在一条连上了的连接上把一句话说完（施工 3-9 下）：握手、找会话、订阅、发，跟着那一轮边收边打。

use serde_json::json;
use tokio::sync::mpsc;

use miyu_ipc::Connection;

use super::follow::{Follow, Step};
use super::{Plan, Screen, Target, exit};
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
    mut presses: mpsc::Receiver<()>,
) -> u8 {
    let mut rpc = Rpc::new(connection, "ask");
    // 一律说没人能确认：`miyu ask` 里没有确认的界面（`22-命令行.md` O3，2026-09-28 项目主人改），要问人的当场
    // 拒绝、告诉她原因，不一直等着。
    if let Err(code) = link::hello(&mut rpc, token, &plan.language, false, screen.err).await {
        return code;
    }
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
    let send = json!({"session": session, "text": plan.text, "cwd": plan.cwd});
    let sent = match rpc.send("session.send", send).await {
        Ok(sent) => sent,
        Err(error) => {
            say(screen.err, &error.to_string());
            return exit::ERROR;
        }
    };
    let mut follow = Follow::new(&session, &sent, plan);
    // 造会话的回应里说了会话实际在哪个目录里干活：目录太宽的，第一步之前说一句（施工 4-5 下）。
    if let Some(used) = &used {
        follow.moved(used, screen);
    }
    let mut interrupting = false;
    loop {
        tokio::select! {
            message = rpc.next() => {
                let Some(message) = message else {
                    say(screen.err, &plan.language.disconnected());
                    return exit::ERROR;
                };
                match follow.take(&message, screen) {
                    Step::Going => {}
                    Step::Done(code) => return code,
                    Step::Resubscribe => {
                        if rpc.send("subscribe", subscribe.clone()).await.is_err() {
                            say(screen.err, &plan.language.disconnected());
                            return exit::ERROR;
                        }
                    }
                }
            }
            Some(()) = presses.recv() => {
                if interrupting {
                    say(screen.err, &plan.language.interrupted());
                    return exit::INTERRUPTED;
                }
                interrupting = true;
                let interrupt = json!({"session": session, "queued": "return"});
                if rpc.send("session.interrupt", interrupt).await.is_err() {
                    say(screen.err, &plan.language.disconnected());
                    return exit::ERROR;
                }
            }
        }
    }
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
            let params = json!({"cwd": plan.cwd, "oneshot": true});
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
