//! 标题那一句（`docs/blueprint/view.md`「标题那一句」，终端蓝图 `tui.md`「时间线」第 3、6–9 条）：显示名、对象、
//! 别的会话的短编号、结果那一句，照连接的语言。

use serde_json::Value;

use miyu_kernel::event::Said;
use miyu_kernel::id::SessionId;

use crate::entry::{Title, ToolState};
use crate::words::{self, Texts, ToolKind, keys};

/// 一步工具的标题。`said` 是结果那一句的说法，结果还没到的没有。
pub(crate) fn of(
    name: &str,
    args: &Value,
    state: ToolState,
    said: Option<&Said>,
    texts: &Texts,
) -> Title {
    let words = texts.local.as_ref();
    let face = words.face(name);
    let shown = face.as_ref().map_or(name, |f| f.name.as_str()).to_string();
    if state == ToolState::Preparing {
        let name = words::say(words, keys::PREPARE, &[("name", shown.clone())]).unwrap_or(shown);
        return Title {
            name,
            ..Title::default()
        };
    }
    let kind = texts.kinds.of(name);
    let arg = |key: &str| {
        args.get(key)
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
    };
    let object = match kind {
        Some(ToolKind::Command | ToolKind::Agent) => arg("description").map(str::to_string),
        Some(ToolKind::Message) => arg("to").map(|to| recipient(to, texts)),
        _ => face
            .as_ref()
            .and_then(|f| f.subject.as_deref())
            .and_then(arg)
            .map(|value| home_short(value, texts.home.as_deref())),
    };
    let session = texts
        .kinds
        .session_arg
        .as_deref()
        .and_then(arg)
        .map(|id| on_session(id, texts));
    // 别的工具后面跟结果那一句；命令有预览、编辑有加减的行数、子代理有任务编号，不写；留言送到了的和对象重了，不写。
    let said = match kind {
        None => said.and_then(|s| words.say(s)),
        Some(ToolKind::Message) if state.failed() => said.and_then(|s| words.say(s)),
        _ => None,
    };
    Title {
        name: shown,
        object,
        session,
        said,
    }
}

/// 留言发给谁：子代理写任务编号，父会话写「父会话」，别的会话写「会话 短编号」。
fn recipient(to: &str, texts: &Texts) -> String {
    if to == "parent" {
        return words::say(texts.local.as_ref(), keys::PARENT, &[])
            .unwrap_or_else(|| to.to_string());
    }
    match SessionId::parse(to) {
        Ok(_) => on_session(to, texts),
        Err(_) => to.to_string(),
    }
}

/// 「会话 短编号」：短编号是整个编号的最后 8 个字。
fn on_session(id: &str, texts: &Texts) -> String {
    let short = short(id);
    words::say(
        texts.local.as_ref(),
        keys::ON_SESSION,
        &[("id", short.clone())],
    )
    .unwrap_or(short)
}

/// 短编号：整个编号的最后 8 个字（核心 C-1 定的）。
pub(crate) fn short(id: &str) -> String {
    let skip = id.chars().count().saturating_sub(8);
    id.chars().skip(skip).collect()
}

/// 在家目录底下的路径写成 `~/…`。
fn home_short(path: &str, home: Option<&str>) -> String {
    let Some(home) = home
        .map(|h| h.trim_end_matches('/'))
        .filter(|h| !h.is_empty())
    else {
        return path.to_string();
    };
    match path.strip_prefix(home) {
        Some("") => "~".to_string(),
        Some(rest) if rest.starts_with('/') => format!("~{rest}"),
        _ => path.to_string(),
    }
}
