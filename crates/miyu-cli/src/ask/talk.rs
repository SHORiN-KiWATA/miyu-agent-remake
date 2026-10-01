//! 在一条连上了的连接上把一句话说完（施工 3-9 下）：握手、传附件（施工 3-9 三补）、找会话、订阅、发，跟着那一轮边收
//! 边打；她派了子代理的，接着等它们报回来（施工 7-9）。跟着那一轮的那一段（[`follow_turn`]）`miyu compact` 也用（施工 6-8）。

use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio::time::Instant;

use miyu_ipc::Connection;

use super::follow::{Follow, Leaving, Step};
use super::{Plan, Screen, Target, exit};
use crate::language::Language;
use crate::link;
use crate::rpc::Rpc;
use crate::shown::say;

/// 握手、找会话、订阅、发，跟着那一轮边收边打，交回退出码。`presses` 是一次次的 Ctrl+C：第一次打断这一轮
/// （排着的退回），等它收尾；第二次不等了。她派了子代理的，等它们都报回来、被叫醒的几轮也结束（施工 7-9）：等的时候按
/// Ctrl+C 是不等了；`plan.timeout` 从发出算起，到了打断、不再等。
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
    let hello = match link::hello(&mut rpc, token, &plan.language, false, screen.err).await {
        Ok(hello) => hello,
        Err(code) => return code,
    };
    let unsandboxed = link::unsandboxed(&hello);
    let config_errors = link::config_errors(&hello);
    // 握手以后照核心回的语言说（施工 8-2）：换了的，给人看的字也照新的那种读一份。
    let spoken;
    let plan = match link::spoken(&hello, plan.language) {
        language if language == plan.language => plan,
        language => {
            spoken = Plan {
                language,
                human: super::human(&miyu_store::env::Env::current(), &language),
                ..plan.clone()
            };
            &spoken
        }
    };
    // 附件在造会话之前传：传不上的不发话，也不留下一个空的会话（施工 3-9 三补）。
    let attachments = match attach(&mut rpc, plan, screen).await {
        Ok(attachments) => attachments,
        Err(code) => return code,
    };
    let (session, created) = match session(&mut rpc, plan, screen).await {
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
    let send = send_params(&session, plan, attachments);
    // 从发出算起（施工 7-9）；长到算不出那一刻的，当没写。
    let deadline = plan
        .timeout
        .and_then(|timeout| Instant::now().checked_add(timeout));
    let sent = match rpc.send("session.send", send).await {
        Ok(sent) => sent,
        Err(error) => {
            say(screen.err, &error.to_string());
            return exit::ERROR;
        }
    };
    let mut follow = Follow::new(&session, &sent, plan);
    follow.waits();
    // 握手的回应说配置里有错：最先说一句，在沙盒用不了那一句前面（施工 8-2）。
    if config_errors > 0 {
        follow.config_errors(config_errors, screen);
    }
    // 造会话的回应说这里的项目配置还没信任：接着说一句（施工 8-3）。接着说的会话在说话的回应里说。
    if let Some(file) = created["untrusted_project"].as_str() {
        follow.untrusted(file, screen);
    }
    // 握手的回应说沙盒用不了：执行命令都要确认，这里确认不了，最先说一句（施工 5-4 下）。
    if let Some(reason) = &unsandboxed {
        follow.unsandboxed(reason, screen);
    }
    // 造会话的回应里说了会话实际在哪个目录里干活：目录太宽的，第一步之前说一句（施工 4-5 下）。
    if let Some(used) = created["cwd"].as_str() {
        follow.moved(used, screen);
    }
    let watching = Watching {
        subscribe: &subscribe,
        session: &session,
        queued: "return",
        language: &plan.language,
        deadline,
    };
    follow_turn(&mut rpc, &mut follow, &watching, screen, presses).await
}

/// 跟着一轮时要的几样：重新订阅用的参数、哪个会话、打断时排着的怎么办（`return` 退回、`send` 接着发）、界面语言、最多等到
/// 什么时候（`--timeout`，施工 7-9；没有的一直等）。
pub(crate) struct Watching<'a> {
    pub(crate) subscribe: &'a Value,
    pub(crate) session: &'a str,
    pub(crate) queued: &'a str,
    pub(crate) language: &'a Language,
    pub(crate) deadline: Option<Instant>,
}

/// 跟着那一轮边收边打，交回退出码（施工 6-8 从 [`talk`] 拆出来，`miyu compact` 也用）：收到 `resync` 重新订阅，不补看
/// 掉的那些；第一次 Ctrl+C 打断这一轮，排着的照 `watching.queued` 办，等它收尾；第二次不等了，说「打断了」。等子代理的
/// 时候（施工 7-9）按 Ctrl+C 不等了；到了 `watching.deadline`，有回合在进行的打断它，不再等。别的 harness 说的（`--from`，
/// 施工 7-10）两样都不打断，只是不等了。
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
        deadline,
    } = watching;
    let interrupt = json!({"session": session, "queued": queued});
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
                // 等子代理的时候、别的 harness 说的（施工 7-10）：不等了，不打断。
                if follow.waiting() || !follow.interrupts() {
                    return follow.leave(Leaving::Pressed, screen);
                }
                if interrupting {
                    say(screen.err, &language.interrupted());
                    return exit::INTERRUPTED;
                }
                interrupting = true;
                if rpc.send("session.interrupt", interrupt.clone()).await.is_err() {
                    say(screen.err, &language.disconnected());
                    return exit::ERROR;
                }
            }
            () = until(*deadline) => {
                // 有回合在进行（还没认出第一轮的也当它在进行）：叫它打断，不等它收尾；别的 harness 说的不打断（施工 7-10）。发不
                // 出去的，核心已经断开了。
                if !follow.waiting()
                    && follow.interrupts()
                    && rpc.send("session.interrupt", interrupt.clone()).await.is_err()
                {
                    say(screen.err, &language.disconnected());
                    return exit::ERROR;
                }
                return follow.leave(Leaving::TimedOut, screen);
            }
        }
    }
}

/// 等到 `deadline`；没有的一直等。
async fn until(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}

/// `session.send` 的参数。本人说的带 `cwd`、`dirs`，`dirs` 每次都写；别的 harness 说的（`--from`，施工 7-10）带 `from`，不带
/// `cwd`，`dirs` 只在写了 `--add-dir` 时带：会话的工作目录、加进来的目录是人的，别的 harness 发一句不该把它们换成自己的
/// （2026-09-30 主会话定）。有附件的再带上附件，照 `blob.put` 的回应原样放。
fn send_params(session: &str, plan: &Plan, attachments: Vec<Value>) -> Value {
    let mut send = json!({"session": session, "text": plan.text});
    match &plan.from {
        None => {
            send["cwd"] = json!(plan.cwd);
            send["dirs"] = json!(plan.dirs);
        }
        Some(from) => {
            send["from"] = json!(from);
            if !plan.dirs.is_empty() {
                send["dirs"] = json!(plan.dirs);
            }
        }
    }
    if !attachments.is_empty() {
        send["attachments"] = Value::Array(attachments);
    }
    send
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

/// 接哪个会话：新开一个一次性的；上一次 `miyu ask` 开的；指定的。交回会话的编号；新开的，再交回造会话的回应：核心说的它
/// 实际在哪个目录里干活、这里的项目配置没信任（施工 8-3），别的是 `null`。写了 `--model` 的（施工 8-10）：新开的照它造，
/// 接着的先换成它（`session.configure`，永久换）；换不成的照核心的原话说，不发话。
async fn session(
    rpc: &mut Rpc,
    plan: &Plan,
    screen: &mut Screen<'_>,
) -> Result<(String, Value), u8> {
    let session = match &plan.target {
        Target::New => {
            let mut params = json!({"cwd": plan.cwd, "dirs": plan.dirs, "oneshot": true});
            if let Some(model) = &plan.model {
                params["model"] = json!(model);
            }
            let result =
                link::request(rpc, "session.create", params, &plan.language, screen.err).await?;
            let session = result["session"].as_str().unwrap_or_default().to_string();
            return Ok((session, result));
        }
        Target::Continue => link::latest_oneshot(rpc, &plan.language, screen.err).await?,
        Target::Session(session) => session.clone(),
    };
    if let Some(model) = &plan.model {
        let params = json!({"session": session, "model": model});
        link::request(rpc, "session.configure", params, &plan.language, screen.err).await?;
    }
    Ok((session, Value::Null))
}
