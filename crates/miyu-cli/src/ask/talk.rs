//! 在一条连上了的连接上把一句话说完（施工 3-9 下）：握手、找会话、订阅、发，跟着那一轮边收边打。

use serde_json::{Value, json};
use tokio::sync::mpsc;

use miyu_ipc::Connection;

use super::follow::{Follow, Step, say};
use super::rpc::Rpc;
use super::{Plan, Screen, Target, exit};

/// 握手、找会话、订阅、发，跟着那一轮边收边打，交回退出码。`presses` 是一次次的 Ctrl+C：第一次打断这一轮
/// （排着的退回），等它收尾；第二次不等了。
pub async fn talk(
    connection: Connection,
    token: &str,
    plan: &Plan,
    screen: &mut Screen<'_>,
    mut presses: mpsc::Receiver<()>,
) -> u8 {
    let mut rpc = Rpc::new(connection);
    let hello = json!({
        "protocol": [1, 1],
        "head": {"kind": "cli", "version": env!("CARGO_PKG_VERSION")},
        "locale": plan.language.locale(),
        "caps": {"input": plan.input},
        "token": token,
    });
    if let Err(code) = request(&mut rpc, "hello", hello, plan, screen).await {
        return code;
    }
    let (session, used) = match session(&mut rpc, plan, screen).await {
        Ok(found) => found,
        Err(code) => return code,
    };
    let subscribe = json!({"session": session, "stream": "events"});
    if let Err(code) = request(&mut rpc, "subscribe", subscribe.clone(), plan, screen).await {
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
            let result = request(rpc, "session.create", params, plan, screen).await?;
            let session = result["session"].as_str().unwrap_or_default().to_string();
            Ok((session, result["cwd"].as_str().map(str::to_string)))
        }
        Target::Continue => {
            let params = json!({"oneshot": true, "limit": 1});
            let result = request(rpc, "session.list", params, plan, screen).await?;
            match result["sessions"][0]["session"].as_str() {
                Some(session) => Ok((session.to_string(), None)),
                None => {
                    say(screen.err, &plan.language.no_oneshot());
                    Err(exit::ERROR)
                }
            }
        }
        Target::Session(session) => Ok((session.clone(), None)),
    }
}

/// 发一条请求，等回应，交回 `result`。被拒绝的、核心断开的，说清楚，交回退出码。
async fn request(
    rpc: &mut Rpc,
    method: &str,
    params: Value,
    plan: &Plan,
    screen: &mut Screen<'_>,
) -> Result<Value, u8> {
    match rpc.call(method, params).await {
        Ok(Some(reply)) => match reply.get("error") {
            None => Ok(reply["result"].clone()),
            Some(error) => {
                let reason = error["message"].as_str().unwrap_or_default();
                say(screen.err, &plan.language.refused(reason));
                Err(exit::ERROR)
            }
        },
        Ok(None) => {
            say(screen.err, &plan.language.disconnected());
            Err(exit::ERROR)
        }
        Err(error) => {
            say(screen.err, &error.to_string());
            Err(exit::ERROR)
        }
    }
}
