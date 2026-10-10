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
    // 有确认、提问的抽屉：说这一头有人能当场回答（核心 D-1，`session.answer`）。
    let hello = json!({
        "protocol": [1, 1],
        "head": {"kind": "tui", "version": env!("CARGO_PKG_VERSION")},
        // 系统的语言（核心 8-2：`ui.language` 是 `auto` 时核心照它说话），照 `miyu_store::env::locale`，和命令行认的一样。
        "locale": miyu_store::env::locale().unwrap_or_default(),
        "caps": {"input": true},
        "token": token,
    });
    rpc.call("hello", hello).await.map_err(refused)?;
    Ok(rpc)
}

/// 开会话时带上的：`/model` 选的模型、人格框、预设框选的（没选的不写，核心照默认），`/workspace` 换的目录。
#[derive(Debug, Default)]
pub(super) struct Chosen {
    pub model: Option<String>,
    pub persona: Option<String>,
    pub preset: Option<String>,
    /// 验过的工作区（核心 9-7 下）；没换的照终端所在的目录。
    pub cwd: Option<String>,
}

/// 开一个会话：工作目录照换过的、没换的照终端现在的，带上选了的模型、人格、预设（核心 8-8、P-1 上、P-2 上）。
pub(super) async fn create(rpc: &mut Rpc, chosen: &Chosen) -> Result<String, Update> {
    let params = create_params(chosen, cwd);
    let created = rpc.call("session.create", params).await.map_err(refused)?;
    Ok(created["session"].as_str().unwrap_or_default().to_string())
}

/// `session.create` 的参数。`/workspace` 换过的目录带 `chosen: true`：人明着选的，太宽（`~` 这类）也照用；没换的照终端
/// 现在的目录（`here`），不带，太宽的核心照旧退回默认工作区（核心 9-7 补，2026-10-09 项目主人选的 A；「新会话」第 4 条）。
fn create_params(chosen: &Chosen, here: impl FnOnce() -> String) -> serde_json::Value {
    let mut params = match &chosen.cwd {
        Some(dir) => json!({"cwd": dir, "chosen": true}),
        None => json!({"cwd": here()}),
    };
    for (key, value) in [
        ("model", &chosen.model),
        ("persona", &chosen.persona),
        ("preset", &chosen.preset),
    ] {
        if let Some(value) = value {
            params[key] = json!(value);
        }
    }
    // 选了「无人格」：明着不带人格，不看默认的（核心 P-4 上，`"persona": null`）。
    if chosen.persona.as_deref() == Some("") {
        params["persona"] = serde_json::Value::Null;
    }
    params
}

/// 订阅回应里头要的几样：限额、现在用的模型、人格、预设（以前的会话没有人格、预设的不写）。
#[derive(Debug)]
pub(super) struct Joined {
    pub limits: Limits,
    pub current: Option<super::Current>,
    pub persona: Option<String>,
    pub preset: Option<String>,
    /// 累计用量、权限、还在跑的（核心 9-6 上）：重连以后照它换，断开那一段漏掉的补回来。
    pub snapshot: Option<super::Snapshot>,
    /// 会话的工作区（核心 9-7 上）：侧边栏照它写。
    pub workspace: Option<String>,
}

/// 订阅一个会话的事件流，交回订阅的回应里头要的几样。核心重启以后，订阅时才载入这个会话。
pub(super) async fn subscribe(rpc: &mut Rpc, session: &str) -> Result<Joined, Update> {
    let subscribed = rpc
        .call("subscribe", json!({"session": session, "stream": "events"}))
        .await
        .map_err(refused)?;
    let text = |key: &str| subscribed[key].as_str().map(str::to_string);
    Ok(Joined {
        limits: Limits::of(&json!({ "result": subscribed })).unwrap_or_default(),
        current: super::models::current(&subscribed),
        persona: text("persona"),
        preset: text("preset"),
        snapshot: super::Snapshot::read(&subscribed),
        workspace: workspace(&subscribed),
    })
}

/// 订阅回应里会话的工作区（核心 9-7 上）；以前的核心没有。
pub(super) fn workspace(reply: &serde_json::Value) -> Option<String> {
    reply["workspace"]["cwd"].as_str().map(str::to_string)
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

    use super::{Chosen, Update, create_params, start_failed};

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

    #[test]
    fn a_chosen_workspace_is_marked_and_the_terminal_one_is_not() {
        // 2026-10-09 项目主人报：手动 `/workspace ~` 以后开会话，又被退回默认工作区。人明着选的带 `chosen`。
        let here = || "/home/me".to_string();
        let picked = Chosen {
            cwd: Some("/home/me".into()),
            ..Chosen::default()
        };
        assert_eq!(
            create_params(&picked, here),
            serde_json::json!({"cwd": "/home/me", "chosen": true})
        );
        assert_eq!(
            create_params(&Chosen::default(), here),
            serde_json::json!({"cwd": "/home/me"}),
            "终端起来时的目录不写 chosen"
        );
    }
}
