//! 人格、预设的文件改了，下一个回合换上（施工 P-1 再补、P-2 下，`docs/blueprint/kernel/session.md`「换策略快照」，
//! `personas.md`「怎么走」第 6 条，`presets.md`「改了文件」）。每个回合开始，照会话的人格、预设的编号把几层重新找一遍、算
//! 指纹，和现在的快照比：一样的什么都不做；不一样的照新的重拼（记忆的范围、有没有人能确认照旧快照；工具面照新的预设重新筛，
//! 以前就有的那几件照旧快照里的原样），核心的字没变才换：存成 blob，策略交给内核放着，交回新快照的哈希。以前造的快照没有
//! 指纹的不换；找不着、写错了的照旧用原来的，记一行运行日志。

use miyu_config::Values;
use miyu_kernel::id::{ContentHash, VenueId};
use miyu_kernel::session::Policy;
use miyu_policy::preset::{Chosen, MEMORY, PresetFile};
use miyu_policy::{Snapshot, ToolEntry};
use miyu_store::blob::Blobs;
use miyu_store::personas::Personas;
use miyu_store::resources::ResourceRoot;
use miyu_tool::Catalog;

use super::Actor;
use crate::TARGET;
use crate::agents::Agents;
use crate::blocking::blocking;
use crate::open::PresetPlaces;
use crate::snapshot::{Parts, build};
use crate::spawn::Lineage;

/// 换快照要的：人格的几层、资源目录、存快照的地方，现在的快照，是不是子会话；预设的几层和装了的软件，重新筛工具面要的目录、
/// 场所、父会话（施工 P-2 下）。
#[derive(Clone)]
pub(crate) struct Refresh {
    pub(crate) personas: Personas,
    pub(crate) resources: ResourceRoot,
    pub(crate) blobs: Blobs,
    pub(crate) snapshot: Snapshot,
    pub(crate) child: bool,
    pub(crate) presets: Option<PresetPlaces>,
    pub(crate) tools: Catalog,
    pub(crate) venue: VenueId,
    pub(crate) lineage: Option<Lineage>,
}

/// 看了一遍的结果。
enum Seen {
    /// 没改，或者以前造的快照没有指纹。
    Same,
    /// 改了，换上这一份：快照、照它造的策略、它的哈希。
    Swapped(Box<Snapshot>, Box<Policy>, ContentHash),
    /// 人格、预设找不着、写错了、读不了：照旧。
    Unreadable(String),
    /// 改了，换不了：程序升级过、拼不成、存不进：照旧。
    Kept(String),
}

impl Actor {
    /// 人格的文件改了，下一个回合换上：造会话、载入时交。
    pub(crate) fn watch_persona(&mut self, refresh: Refresh) {
        self.persona = Some(refresh);
    }

    /// 回合开始：看人格、预设的文件改了没有。改了、换得了的，新策略交给内核放着，交回新快照的哈希。`values` 是这一轮冻结
    /// 的配置：新打开的 `subagent` 照它填能选的池。
    pub(super) async fn refresh_persona(&mut self, values: Values) -> Option<ContentHash> {
        let refresh = self.persona.clone()?;
        let persona = refresh.snapshot.persona.clone();
        match blocking(move || look(&refresh, &values)).await {
            Seen::Same => None,
            Seen::Swapped(snapshot, policy, hash) => {
                tracing::info!(target: TARGET, persona = persona.as_str(), "persona swapped");
                self.session.stage_policy(*policy);
                if let Some(refresh) = self.persona.as_mut() {
                    refresh.snapshot = *snapshot;
                }
                Some(hash)
            }
            Seen::Unreadable(error) => {
                tracing::warn!(target: TARGET, persona = persona.as_str(), error = error.as_str(), "persona unreadable");
                None
            }
            Seen::Kept(error) => {
                tracing::warn!(target: TARGET, persona = persona.as_str(), error = error.as_str(), "persona not swapped");
                None
            }
        }
    }
}

/// 在阻塞线程里看一遍。
fn look(refresh: &Refresh, values: &Values) -> Seen {
    let old = &refresh.snapshot;
    let Some(digest) = &old.persona_digest else {
        return Seen::Same;
    };
    let found = match refresh.personas.find(&old.persona) {
        Ok(found) => found,
        Err(error) => return Seen::Unreadable(error.to_string()),
    };
    let preset = match preset(refresh) {
        Ok(preset) => preset,
        Err(error) => return Seen::Unreadable(error),
    };
    if found.texts.digest() == *digest && preset.is_none() {
        return Seen::Same;
    }
    let (face, pin) = match &preset {
        Some(chosen) => (refaced(refresh, values, &chosen.file), Some(chosen.pin())),
        None => (old.tools.clone(), old.preset.clone()),
    };
    let parts = Parts {
        name: old.persona.clone(),
        texts: found.texts,
        attended: old.attended,
        face,
        memory: old.memory.clone(),
        child: refresh.child,
        preset: pin,
    };
    let new = match build(&refresh.resources, parts) {
        Ok(new) => new,
        Err(error) => return Seen::Kept(error.to_string()),
    };
    if !old.swappable(&new) {
        return Seen::Kept("the core texts changed since this session was made".to_string());
    }
    let policy = match new.policy() {
        Ok(policy) => policy,
        Err(error) => return Seen::Kept(error.to_string()),
    };
    if let Err(error) = refresh.blobs.put(&new.to_bytes()) {
        return Seen::Kept(error.to_string());
    }
    let hash = new.hash();
    Seen::Swapped(Box::new(new), Box::new(policy), hash)
}

/// 照快照里预设的编号重新找一遍（施工 P-2 下）：改了的交回新找到的（记忆照开会话时的），没改的、以前造的快照没有指纹的、
/// 没交预设的几层的是没有。
fn preset(refresh: &Refresh) -> Result<Option<Chosen>, String> {
    let (Some(pin), Some(places)) = (&refresh.snapshot.preset, &refresh.presets) else {
        return Ok(None);
    };
    if pin.digest.is_none() {
        return Ok(None);
    }
    let found = places
        .presets
        .find(&pin.id)
        .map_err(|error| error.to_string())?;
    let installed = || places.installed.iter().map(String::as_str);
    let memory = !pin.off.iter().any(|software| software == MEMORY);
    let chosen = Chosen::new(found.id, found.file, installed()).keeping_memory(memory, installed());
    Ok((chosen.pin() != *pin).then_some(chosen))
}

/// 照新的预设 `file` 重新筛工具面（施工 P-2 下）：场所、子会话、能不能确认、记忆的范围照旧快照；以前就有的那几件照旧快照里
/// 的原样（描述、`subagent` 能选的池都不跟着变），新打开的照现在的目录拿、能选的池照这一轮的配置 `values`。
fn refaced(refresh: &Refresh, values: &Values, file: &PresetFile) -> Vec<ToolEntry> {
    let old = &refresh.snapshot;
    Agents::face(
        &refresh.tools,
        &refresh.venue,
        refresh.lineage.as_ref(),
        values,
        old.attended,
        old.memory_scope(),
        Some(file),
    )
    .into_iter()
    .map(|entry| {
        old.tools
            .iter()
            .find(|kept| kept.name == entry.name)
            .cloned()
            .unwrap_or(entry)
    })
    .collect()
}

#[cfg(test)]
mod preset_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
