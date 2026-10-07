//! 人格的文件改了，下一个回合换上（施工 P-1 再补，`docs/blueprint/kernel/session.md`「换策略快照」，`personas.md`「怎么走」
//! 第 6 条）。每个回合开始，照会话的人格编号把几层重新找一遍、算指纹，和现在的快照比：一样的什么都不做；不一样的照新的字
//! 重拼（工具面、记忆的范围、有没有人能确认照旧快照），除了人格那几格别的都一样才换：存成 blob，策略交给内核放着，交回新
//! 快照的哈希。以前造的快照没有指纹的不换；人格找不着、写错了的照旧用原来的，记一行运行日志。

use miyu_kernel::id::ContentHash;
use miyu_kernel::session::Policy;
use miyu_policy::Snapshot;
use miyu_store::blob::Blobs;
use miyu_store::personas::Personas;
use miyu_store::resources::ResourceRoot;

use super::Actor;
use crate::TARGET;
use crate::blocking::blocking;
use crate::snapshot::{Parts, build};

/// 换快照要的：人格的几层、资源目录、存快照的地方，现在的快照，是不是子会话。
#[derive(Clone)]
pub(crate) struct Refresh {
    pub(crate) personas: Personas,
    pub(crate) resources: ResourceRoot,
    pub(crate) blobs: Blobs,
    pub(crate) snapshot: Snapshot,
    pub(crate) child: bool,
}

/// 看了一遍的结果。
enum Seen {
    /// 没改，或者以前造的快照没有指纹。
    Same,
    /// 改了，换上这一份：快照、照它造的策略、它的哈希。
    Swapped(Box<Snapshot>, Box<Policy>, ContentHash),
    /// 人格找不着、写错了、读不了：照旧。
    Unreadable(String),
    /// 改了，换不了：程序升级过、拼不成、存不进：照旧。
    Kept(String),
}

impl Actor {
    /// 人格的文件改了，下一个回合换上：造会话、载入时交。
    pub(crate) fn watch_persona(&mut self, refresh: Refresh) {
        self.persona = Some(refresh);
    }

    /// 回合开始：看人格的文件改了没有。改了、换得了的，新策略交给内核放着，交回新快照的哈希。
    pub(super) async fn refresh_persona(&mut self) -> Option<ContentHash> {
        let refresh = self.persona.clone()?;
        let persona = refresh.snapshot.persona.clone();
        match blocking(move || look(&refresh)).await {
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
fn look(refresh: &Refresh) -> Seen {
    let old = &refresh.snapshot;
    let Some(digest) = &old.persona_digest else {
        return Seen::Same;
    };
    let found = match refresh.personas.find(&old.persona) {
        Ok(found) => found,
        Err(error) => return Seen::Unreadable(error.to_string()),
    };
    if found.texts.digest() == *digest {
        return Seen::Same;
    }
    let parts = Parts {
        name: old.persona.clone(),
        texts: found.texts,
        attended: old.attended,
        face: old.tools.clone(),
        memory: old.memory.clone(),
        child: refresh.child,
        preset: old.preset.clone(),
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

#[cfg(test)]
mod tests;
