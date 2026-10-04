//! 连上核心、握手、开会话、订阅（蓝图 `tui.md`「连核心」第 2–4、7、8 条）。断了以后重连也走这里：每次拉起核心
//! 都换令牌，连上以后再读（`miyu_ipc` 连的时候读）。

use std::io;
use std::path::Path;
use std::process::Command as Process;

use serde_json::json;

use miyu_ipc::{ConnectError, StartError};
use miyu_store::env::Env;
use miyu_store::root::DataRoot;

use super::Update;
use super::limits::Limits;
use super::rpc::{Failure, Rpc};

/// 连上、握手：给了 `MIYU_CORE_BIN` 的，核心没在跑就拉起来；没给的只连。
pub(super) async fn connect() -> Result<Rpc, Update> {
    let env = Env::current();
    let root = DataRoot::locate(&env).map_err(|e| Update::Failed(e.to_string()))?;
    root.prepare().map_err(|e| Update::Failed(e.to_string()))?;
    let connected = match std::env::var_os("MIYU_CORE_BIN") {
        Some(bin) => {
            let path = Path::new(&bin).to_path_buf();
            miyu_ipc::connect_or_start(&root, move || {
                let mut core = Process::new(bin);
                core.arg("core");
                core
            })
            .await
            .map_err(|e| start_failed(e, &path))
        }
        None => miyu_ipc::connect(&root).await.map_err(|e| match e {
            ConnectError::NotRunning => Update::NoCoreBin,
            e => Update::Failed(e.to_string()),
        }),
    };
    let (connection, token) = connected?;
    let mut rpc = Rpc::new(connection);
    // 还没有确认的抽屉，先说没人能当场回答：要确认的那一步，核心当场拒绝，不会一直等着。
    let hello = json!({
        "protocol": [1, 1],
        "head": {"kind": "tui", "version": env!("CARGO_PKG_VERSION")},
        // 系统的语言（核心 8-2：`ui.language` 是 `auto` 时核心照它说话），照 `miyu_store::env::locale`，和命令行认的一样。
        "locale": miyu_store::env::locale().unwrap_or_default(),
        "caps": {"input": false},
        "token": token,
    });
    rpc.call("hello", hello).await.map_err(refused)?;
    Ok(rpc)
}

/// 开一个会话（`cwd` 是启动时的目录），交回编号。
pub(super) async fn create(rpc: &mut Rpc, model: Option<&str>) -> Result<String, Update> {
    let mut params = json!({"cwd": cwd()});
    // `/new` 以后、开会话以前在 `/model` 选的：开会话时带上（核心 8-8）。
    if let Some(model) = model {
        params["model"] = json!(model);
    }
    let created = rpc.call("session.create", params).await.map_err(refused)?;
    Ok(created["session"].as_str().unwrap_or_default().to_string())
}

/// 订阅一个会话的事件流，交回订阅的回应里的限额和会话现在用的模型。核心重启以后，订阅时才载入这个会话。
pub(super) async fn subscribe(
    rpc: &mut Rpc,
    session: &str,
) -> Result<(Limits, Option<super::Current>), Update> {
    let subscribed = rpc
        .call("subscribe", json!({"session": session, "stream": "events"}))
        .await
        .map_err(refused)?;
    let limits = Limits::of(&json!({ "result": subscribed })).unwrap_or_default();
    Ok((limits, super::models::current(&subscribed)))
}

/// 请求没成：连接断了的说断开，别的说连不上和原因。
pub(super) fn refused(failure: Failure) -> Update {
    match failure {
        Failure::Io(e) => Update::Failed(e.to_string()),
        Failure::Disconnected => Update::Disconnected,
        Failure::Refused(reason) => Update::Failed(reason),
    }
}

/// 拉不起核心：`bin` 指的程序不存在，说清是哪个路径（第 8 条）；别的照原样。
fn start_failed(error: StartError, bin: &Path) -> Update {
    match error {
        StartError::Io(e) if e.kind() == io::ErrorKind::NotFound && !bin.exists() => {
            Update::Missing(bin.display().to_string())
        }
        e => Update::Failed(e.to_string()),
    }
}

/// 启动时的目录，读不出来的写 `.`（照 `miyu ask`）。
pub(super) fn cwd() -> String {
    std::env::current_dir().map_or_else(|_| ".".to_string(), |d| d.display().to_string())
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::path::Path;

    use miyu_ipc::StartError;

    use super::{Update, start_failed};

    #[test]
    fn a_missing_core_program_names_its_path() {
        let gone = Path::new("/nonexistent/miyu");
        let not_found = || StartError::Io(io::Error::from(io::ErrorKind::NotFound));
        assert!(
            matches!(start_failed(not_found(), gone), Update::Missing(p) if p == "/nonexistent/miyu")
        );
        // 程序在，别的东西没找到：照原样。
        let here = Path::new("/");
        assert!(matches!(start_failed(not_found(), here), Update::Failed(_)));
        assert!(matches!(
            start_failed(StartError::Busy, gone),
            Update::Failed(_)
        ));
    }
}
