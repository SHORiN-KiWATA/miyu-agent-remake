//! 用不了的命令为什么用不了（施工 O-6 再补，2026-10-09 项目主人在网页上验收：带了人格、预设没开记忆的会话打 `/remember`，
//! 不知道为什么）：`command.catalog` 的 `unavailable` 和 `command.run` 拒绝的 `data.why` 是同一句，照连接的语言。记忆用不了的
//! 说具体：会话没有人格；会话的预设没开记忆，带上预设的名字，人才知道去哪改。别的照拒绝的那一句。

use miyu_policy::preset::MEMORY;
use miyu_session::Handle;

use super::{Said, words};
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// `refused` 这一句拒绝，说成给人看的为什么。
pub(super) async fn why(core: &Core, peer: &Peer, handle: &Handle, refused: &Refusal) -> String {
    if refused.reason == Refusal::MEMORY_UNAVAILABLE.reason
        && let Some(said) = memory(core, peer, handle).await
    {
        let text = words(core, peer, &said).await;
        if !text.is_empty() {
            return text;
        }
    }
    refused.message(peer.locale).to_string()
}

/// 记忆为什么用不了：没有人格的、预设没开记忆的；会话表里查不到、说不准的是 `None`，照拒绝的那一句。
async fn memory(core: &Core, peer: &Peer, handle: &Handle) -> Option<Said> {
    let (root, admin, index) = (core.root.clone(), core.admin.clone(), core.index.clone());
    let id = handle.id().clone();
    let listed =
        tokio::task::spawn_blocking(move || crate::list::one(&root, &admin, &index, &id, false))
            .await
            .ok()??;
    if listed.persona.is_none() {
        return Some(Said::plain("commands/unavailable/no-persona"));
    }
    let preset = listed.preset?;
    let found = crate::presets::resolve(core, Some(&preset)).await.ok()?;
    if found.file.opens(MEMORY) {
        return None;
    }
    let name = crate::personas::label(found.file.name.as_ref(), peer.language).unwrap_or(preset);
    Some(Said {
        key: "commands/unavailable/preset-off",
        fields: vec![("preset", name)],
    })
}
