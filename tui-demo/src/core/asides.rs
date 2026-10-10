//! 不对着会话的请求：列会话、置顶、删会话、模型资料、给人看的字、写配置、读后台命令的输出。一个命令一条请求，
//! 有的回应要另外办（[`Awaiting`]）。照这张表发，`serve.rs` 不再一个个写。

use serde_json::{Value, json};

use super::awaiting::Awaiting;
use super::{Command, config};

/// 这个命令是不是一条不对着会话的请求：方法、参数、回应要不要另外办。不是的交回 `None`。
pub(super) fn request(command: &Command) -> Option<(&'static str, Value, Option<Awaiting>)> {
    Some(match command {
        Command::ListSessions => ("session.list", json!({}), Some(Awaiting::List)),
        Command::Pin { session, pinned } => (
            "session.set_meta",
            json!({"session": session, "pinned": pinned}),
            None,
        ),
        Command::Delete(session) => ("session.delete", json!({"session": session}), None),
        Command::ListModels => ("model.list", json!({}), Some(Awaiting::Models)),
        Command::ListChoices => ("model.list", json!({}), Some(Awaiting::Choices)),
        Command::SetChat(reference) => ("config.set", config::set_chat(reference), None),
        Command::LinkPreview(url) => (
            "link.preview",
            json!({"url": url}),
            Some(Awaiting::LinkPreview(url.clone())),
        ),
        Command::RenderMermaid(source) => (
            "mermaid.render",
            json!({"source": source}),
            Some(Awaiting::Mermaid(source.clone())),
        ),
        Command::ListEfforts => ("model.list", json!({}), Some(Awaiting::Efforts)),
        Command::SetEffort { key, level } => (
            "config.set",
            config::set_effort(key, level.as_deref()),
            None,
        ),
        Command::Files {
            word,
            method,
            params,
        } => (method, params.clone(), Some(Awaiting::Files(word.clone()))),
        Command::FetchHuman(code) => (
            "human.get",
            json!({"language": code}),
            Some(Awaiting::Human),
        ),
        Command::SetLanguage(code) => ("config.set", config::set_language(code), None),
        // 读输出对着派它的那个会话，不管现在对着哪个（`protocol.md` 的 `job.output`）。
        Command::Output { session, job, tail } => (
            "job.output",
            json!({"session": session, "job": job, "tail": tail}),
            Some(Awaiting::Output(session.clone(), job.clone())),
        ),
        Command::ListPersonas => ("persona.list", json!({}), Some(Awaiting::Personas)),
        Command::ListPresets => ("preset.list", json!({}), Some(Awaiting::Presets)),
        Command::ListPackages => ("package.list", json!({}), Some(Awaiting::Packages)),
        Command::Usage(ask) => ("usage.query", ask.params(), Some(Awaiting::Usage(ask.kind))),
        Command::Ask {
            tag,
            method,
            params,
        } => (*method, params.clone(), Some(Awaiting::Ask(*tag))),
        _ => return None,
    })
}
