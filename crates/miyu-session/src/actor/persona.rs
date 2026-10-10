//! 人格、预设的文件改了，包的工具变了，下一个回合换上（施工 P-1 再补、P-2 下、O-2 中，`docs/blueprint/kernel/session.md`
//! 「换策略快照」，`personas.md`「怎么走」第 6 条，`presets.md`「改了文件」，`providers.md`「目录换代」）。每个回合开始，照会话的
//! 人格、预设的编号把几层重新找一遍、算指纹，看工具目录的架子换没换代，和现在的快照比：一样的什么都不做；不一样的照新的重拼
//! （记忆的范围、有没有人能确认照旧快照；工具面照新的预设、现在的目录重新筛，提供者的工具照现在的登记，别的以前就有的照旧
//! 快照里的原样），拼出来不一样、核心的字没变才换：存成 blob，策略交给内核放着，交回新快照的哈希。以前造的快照没有指纹的
//! 不换；找不着、写错了的照旧用原来的，记一行运行日志。换的时候执行器手里取自快照的几份字（驱动的占位、替工具写的两句、
//! 权限策略的几句）一起换成新快照的（施工 P-1 三补）：升级过的老会话也换得了；光是升级不换（核心的字变了、人格预设工具面
//! 都没变的照旧），不为它断一次缓存。

use miyu_config::Values;
use miyu_drivers::DriverTexts;
use miyu_kernel::id::{ContentHash, VenueId};
use miyu_kernel::session::Policy;
use miyu_policy::PersonaTexts;
use miyu_policy::features::Features;
use miyu_policy::preset::{Chosen, MEMORY, PresetFile, ROLEPLAY};
use miyu_policy::{GuardTexts, RunTexts, Snapshot, ToolEntry};
use miyu_store::blob::Blobs;
use miyu_store::personas::{PersonaError, Personas};
use miyu_store::resources::ResourceRoot;
use miyu_tool::{Catalog, Edition, Shelf};

use super::Actor;
use crate::TARGET;
use crate::agents::{Agents, Offers, Site};
use crate::blocking::blocking;
use crate::open::PresetPlaces;
use crate::snapshot::{Parts, build, tooled};
use crate::spawn::Lineage;

/// 换快照要的：人格的几层、资源目录、存快照的地方，现在的快照，是不是子会话；预设的几层和装了的软件，重新筛工具面要的目录
/// 架子、场所、父会话（施工 P-2 下、O-2 中）。
#[derive(Clone)]
pub(crate) struct Refresh {
    pub(crate) personas: Personas,
    pub(crate) resources: ResourceRoot,
    pub(crate) blobs: Blobs,
    pub(crate) snapshot: Snapshot,
    pub(crate) child: bool,
    pub(crate) presets: Option<PresetPlaces>,
    pub(crate) tools: Shelf,
    /// 现在的快照照的是架子上的哪一代（施工 O-2 中）：载入的不知道是哪一代，是没有，第一个回合照现在的对一次。
    pub(crate) seen: Option<Edition>,
    pub(crate) venue: VenueId,
    pub(crate) lineage: Option<Lineage>,
}

/// 看了一遍的结果。
enum Seen {
    /// 没改，或者以前造的快照没有指纹。
    Same,
    /// 改了，换上这一份：快照、照它造的策略、它的哈希，执行器的几份字（施工 P-1 三补）。
    Swapped(Box<Snapshot>, Box<Policy>, ContentHash, Box<Retext>),
    /// 人格、预设找不着、写错了、读不了：照旧。
    Unreadable(String),
    /// 改了，换不了：拼不成、存不进：照旧。
    Kept(String),
}

impl Actor {
    /// 人格的文件改了，下一个回合换上：造会话、载入时交。
    pub(crate) fn watch_persona(&mut self, refresh: Refresh) {
        self.persona = Some(refresh);
    }

    /// 回合开始：看人格、预设的文件改了没有，工具目录换没换代。改了、换得了的，新策略交给内核放着，交回新快照的哈希。
    /// `values` 是这一轮冻结的配置：新打开的 `subagent` 照它填能选的池。
    pub(super) async fn refresh_persona(&mut self, values: Values) -> Option<ContentHash> {
        let refresh = self.persona.clone()?;
        let persona = refresh.snapshot.persona.clone();
        let now = refresh.tools.edition();
        let seen = now.clone();
        match blocking(move || look(&refresh, &values, &now)).await {
            Seen::Same => {
                if let Some(refresh) = self.persona.as_mut() {
                    refresh.seen = Some(seen);
                }
                None
            }
            Seen::Swapped(snapshot, policy, hash, texts) => {
                tracing::info!(target: TARGET, persona = persona.as_deref(), "persona swapped");
                self.session.stage_policy(*policy);
                let Retext { driver, run, guard } = *texts;
                self.model.retext(driver);
                self.tools.lettering().set(run, guard);
                if let Some(refresh) = self.persona.as_mut() {
                    refresh.snapshot = *snapshot;
                    refresh.seen = Some(seen);
                }
                Some(hash)
            }
            Seen::Unreadable(error) => {
                tracing::warn!(target: TARGET, persona = persona.as_deref(), error = error.as_str(), "persona unreadable");
                None
            }
            Seen::Kept(error) => {
                tracing::warn!(target: TARGET, persona = persona.as_deref(), error = error.as_str(), "persona not swapped");
                None
            }
        }
    }
}

/// 在阻塞线程里看一遍：`now` 是架子上现在的那一代。
fn look(refresh: &Refresh, values: &Values, now: &Edition) -> Seen {
    let old = &refresh.snapshot;
    // 无人格的（施工 P-4 上）只看预设；人格的文件没了的照快照里的接着用，不换也不报。
    let texts = match (&old.persona, &old.persona_digest) {
        (None, _) => PersonaTexts::default(),
        (Some(_), None) => return Seen::Same,
        (Some(id), Some(_)) => match refresh.personas.find(id) {
            Ok(found) => found.texts,
            Err(PersonaError::NotFound(_)) => return Seen::Same,
            Err(error) => return Seen::Unreadable(error.to_string()),
        },
    };
    let preset = match preset(refresh) {
        Ok(preset) => preset,
        Err(error) => return Seen::Unreadable(error),
    };
    let same = old
        .persona_digest
        .as_ref()
        .is_none_or(|digest| texts.digest() == *digest);
    // 目录换了代：预设找得回来的（或者本来没有预设的）照现在的目录重新筛；以前造的、找不回来的照旧（施工 O-2 中）。
    let moved = refresh
        .seen
        .as_ref()
        .is_none_or(|seen| seen.generation != now.generation)
        && !matches!(preset, Preset::Unknown);
    if same && !moved && !matches!(preset, Preset::Changed(_)) {
        return Seen::Same;
    }
    let catalog = &now.catalog;
    let (face, pin) = match &preset {
        Preset::Changed(chosen) => (
            refaced(refresh, values, catalog, &chosen.file),
            Some(chosen.pin()),
        ),
        Preset::Same(chosen) if moved => (
            followed(refresh, values, catalog, Some(&chosen.file)),
            old.preset.clone(),
        ),
        Preset::Nothing if moved => (followed(refresh, values, catalog, None), old.preset.clone()),
        _ => (old.tools.clone(), old.preset.clone()),
    };
    let parts = Parts {
        name: old.persona.clone(),
        texts,
        attended: old.attended,
        face,
        memory: old.memory.clone(),
        child: refresh.child,
        preset: pin,
        tooled: tooled(catalog, features(refresh)),
        roleplay: features(refresh).is_none_or(|features| features.installed(ROLEPLAY)),
        // 群会话照旧快照钉下的时区（施工 O-13 中）：换了时区的机器上换人格，前缀里的钟点也不变。
        group: old.group.as_ref().map(|chat| chat.offset),
        // 换了预设的照新的定后台运行（施工 T-1 上），别的照旧。
        foreground: match &preset {
            Preset::Changed(chosen) => Agents::foreground(Some(&chosen.file)),
            _ => old.foreground,
        },
    };
    let new = match build(&refresh.resources, parts) {
        Ok(new) => new,
        Err(error) => return Seen::Kept(error.to_string()),
    };
    // 目录换了代、这个会话的工具面没变的（只给群的、预设关着的），什么都不记。
    if new.hash() == old.hash() {
        return Seen::Same;
    }
    // 程序升级过（核心的字变了），人格、预设都没改、工具面也没变的：照旧，不为升级断一次缓存（施工 P-1 三补）。
    let edited = !same || matches!(preset, Preset::Changed(_));
    if !edited && !old.swappable(&new) && new.tools == old.tools {
        return Seen::Same;
    }
    let built = || -> Result<(Policy, Retext), String> {
        let texts = Retext {
            driver: new.driver_texts().map_err(|error| error.to_string())?,
            run: new.run_texts().map_err(|error| error.to_string())?,
            guard: new.guard_texts().map_err(|error| error.to_string())?,
        };
        Ok((new.policy().map_err(|error| error.to_string())?, texts))
    };
    let (policy, texts) = match built() {
        Ok(built) => built,
        Err(error) => return Seen::Kept(error),
    };
    if let Err(error) = refresh.blobs.put(&new.to_bytes()) {
        return Seen::Kept(error.to_string());
    }
    let hash = new.hash();
    Seen::Swapped(Box::new(new), Box::new(policy), hash, Box::new(texts))
}

/// 执行器取自快照的几份字（施工 P-1 三补）：换快照时一起换。
struct Retext {
    /// 驱动的占位：交给请求模型的端口。
    driver: DriverTexts,
    /// 执行工具时替工具写的两句。
    run: RunTexts,
    /// 权限策略的几句。
    guard: GuardTexts,
}

/// 快照里的预设现在是什么样（施工 P-2 下；O-2 中重新筛工具面也要它）。
enum Preset {
    /// 这个会话没有预设。
    Nothing,
    /// 找不回来：以前造的快照没有指纹，没交预设的几层。
    Unknown,
    /// 没改：现在找到的那一份。
    Same(Chosen),
    /// 改了：新找到的（记忆照开会话时的）。
    Changed(Chosen),
}

/// 装了的功能（施工 F-3 上）：没交预设几层的（测试里造的、以前的）没有，当都装着。
fn features(refresh: &Refresh) -> Option<&Features> {
    refresh.presets.as_ref().map(|places| &places.features)
}

/// 照快照里预设的编号重新找一遍（施工 P-2 下）。
fn preset(refresh: &Refresh) -> Result<Preset, String> {
    let Some(pin) = &refresh.snapshot.preset else {
        return Ok(Preset::Nothing);
    };
    let Some(places) = &refresh.presets else {
        return Ok(Preset::Unknown);
    };
    if pin.digest.is_none() {
        return Ok(Preset::Unknown);
    }
    let found = places
        .presets
        .find(&pin.id)
        .map_err(|error| error.to_string())?;
    let features = &places.features;
    let memory = !pin.off.iter().any(|feature| feature == MEMORY);
    let chosen = Chosen::new(found.id, found.file, features).keeping_memory(memory, features);
    // 以前的快照记的是包的编号（施工 F-3 上）：照现在的功能读一样的算没改，不为改了写法换一次快照。
    Ok(match pin.means_the_same(&chosen.pin(), features) {
        true => Preset::Same(chosen),
        false => Preset::Changed(chosen),
    })
}

/// 照预设 `file`、现在的目录 `catalog` 筛出给这个会话的工具（施工 P-2 中、O-2 中）：场所、子会话、能不能确认、记忆的范围照旧
/// 快照；新打开的 `subagent` 能选的池照这一轮的配置 `values`。
fn offered(
    refresh: &Refresh,
    values: &Values,
    catalog: &Catalog,
    file: Option<&PresetFile>,
) -> Vec<ToolEntry> {
    let old = &refresh.snapshot;
    Agents::face(
        catalog,
        Site {
            venue: &refresh.venue,
            group: old.group.is_some(),
        },
        refresh.lineage.as_ref(),
        &Offers::of(values, refresh.personas.ids()),
        old.attended,
        old.memory_scope(),
        file.map(|file| (file, features(refresh))),
    )
}

/// 预设改了，照新的预设 `file` 重新筛工具面（施工 P-2 下）：以前就有的那几件照旧快照里的原样（描述、`subagent` 能选的池都
/// 不跟着变，改过名的照旧叫以前的名字），新打开的照现在的目录拿；提供者的工具照现在的登记（施工 O-2 中）。
fn refaced(
    refresh: &Refresh,
    values: &Values,
    catalog: &Catalog,
    file: &PresetFile,
) -> Vec<ToolEntry> {
    let old = &refresh.snapshot;
    offered(refresh, values, catalog, Some(file))
        .into_iter()
        .map(|entry| {
            if catalog.provided(&entry.name) {
                return entry;
            }
            old.tools
                .iter()
                .find(|kept| {
                    catalog
                        .get(&kept.name)
                        .is_some_and(|tool| tool.spec().name == entry.name)
                })
                .cloned()
                .unwrap_or(entry)
        })
        .collect()
}

/// 目录换了代、预设没改（施工 O-2 中）：照旧快照的先后一件一件对现在的目录 `catalog`。提供者的照现在的登记，不再给这个会话
/// 的拿掉；自带的照旧快照里的原样，不新加。目录里没有了的：上一次对过的那一代里是提供者的拿掉（扩展关掉了、不再登记它），
/// 别的照旧留着、调到时暂时不可用（施工 4-2：程序升级拿掉的，载入的会话认不出来的；施工 F-5 中：随包卸掉的，调到时报已卸载）。
/// 新登记的提供者的工具加进来；新装上的内置包（上一次对过的那一代里这个包一件都没有，施工 F-5 中）的工具也加进来，照样过
/// 预设；快照照名字排（`miyu_policy` 的工具面）。
fn followed(
    refresh: &Refresh,
    values: &Values,
    catalog: &Catalog,
    file: Option<&PresetFile>,
) -> Vec<ToolEntry> {
    let fresh = offered(refresh, values, catalog, file);
    let basis = refresh.seen.as_ref().map(|seen| &seen.catalog);
    let mut face = Vec::new();
    for kept in &refresh.snapshot.tools {
        match catalog.get(&kept.name) {
            Some(tool) if catalog.provided(&kept.name) => {
                let name = &tool.spec().name;
                face.extend(fresh.iter().find(|entry| entry.name == *name).cloned());
            }
            Some(_) => face.push(kept.clone()),
            // 随包卸掉的提供者的工具照旧留着、调到时报已卸载（施工 F-5 下）；关掉的扩展的拿掉。
            None if basis.is_some_and(|basis| basis.provided(&kept.name))
                && !catalog.gone(&kept.name) => {}
            None => face.push(kept.clone()),
        }
    }
    // 新装上的内置包：它的包在上一次对过的那一代里一件工具都没有。包里多了一件的（程序升级那种）照旧等预设改了才进来。
    let appeared = |name: &str| {
        basis.is_some_and(|basis| {
            catalog
                .package_of(name)
                .is_some_and(|package| !basis.packages().any(|owner| owner == package))
        })
    };
    let added: Vec<ToolEntry> = fresh
        .into_iter()
        .filter(|entry| {
            (catalog.provided(&entry.name) || appeared(&entry.name))
                && !face.iter().any(|kept| kept.name == entry.name)
        })
        .collect();
    face.extend(added);
    face
}

#[cfg(test)]
mod preset_tests;
#[cfg(test)]
mod shelf_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
