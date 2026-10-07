//! 拼策略快照（施工 3-6 上起在 `open.rs`，施工 P-1 再补挪出来：换快照也照它拼）：人格的字照几层叠好的，接上随核心附带的
//! 字、工具面、记忆的范围；子会话接场所说明；最后是核心的几行和风格锁（`docs/designs/26-提示词.md` 第四节）。

use miyu_policy::{PersonaTexts, Snapshot, ToolEntry, compose};
use miyu_store::resources::{ResourceRoot, SourceError};

/// 拼一份快照的料：人格的编号和叠好的字、有没有人能确认、工具面、记忆的范围（快照里那一格的原样，没有的是没有），是不是
/// 子会话。
pub(crate) struct Parts {
    pub(crate) name: String,
    pub(crate) texts: PersonaTexts,
    pub(crate) attended: bool,
    pub(crate) face: Vec<ToolEntry>,
    pub(crate) memory: Option<String>,
    pub(crate) child: bool,
}

/// 照 `parts` 拼一份快照，随核心附带的字从 `resources` 读这一刻的。
///
/// # Errors
///
/// 资源目录里哪一份读不了。
pub(crate) fn build(resources: &ResourceRoot, parts: Parts) -> Result<Snapshot, SourceError> {
    let sources = resources.sources_with(parts.texts)?;
    let mut snapshot = compose(&parts.name, sources, parts.attended).with_tools(parts.face);
    snapshot.memory = parts.memory;
    if parts.child {
        snapshot = snapshot.with_venue(&resources.subagent_venue()?);
    }
    let lines = resources.core_lines()?;
    Ok(snapshot
        .with_core_lines(&lines)
        .with_style_lock(&lines.style_lock))
}
