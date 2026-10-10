//! 预设（施工 P-2 上，`docs/blueprint/presets.md`，`16-人格与预设.md` 第三节）：`[preset]` 的名字、说明、没列出来的功能开不开，
//! `[features]` 按功能开关（施工 F-3 上，设计 `30-插件框架.md` 第四节），以前的 `[software]` 按软件包开关照认，`[tools]` 关掉
//! 单件工具。纯逻辑：进来的是文件里的字，出去的是读好的样子，或者写明第几行错在哪（`read.rs`）。找哪几层、读盘由存储做。

use std::collections::{BTreeMap, BTreeSet};

use miyu_config::phrases::Label;
use miyu_kernel::id::ContentHash;
use serde::{Deserialize, Serialize};

use crate::features::Features;

mod read;

pub use read::{Code, DEFAULT_PERSONA, Problem, read};

/// 工具名最多几个字符。
const TOOL_CHARS: usize = 64;

/// 人格记忆这个功能（施工 P-2 中，`10-自带软件.md` 第四节；施工 F-3 上起是功能的编号）：三件工具和回合开始的召回。和
/// `miyu_memory::PACKAGE` 是同一个编号：包没写功能，整个包算一个。
pub const MEMORY: &str = "memory";

/// 人设遵循提醒这个功能（施工 P-2 中，原来叫角色扮演；施工 F-3 上起是功能的编号）：人格的提醒短语和风格锁。它没有工具。
pub const ROLEPLAY: &str = "roleplay";

/// 没列在 `[features]`、`[software]` 里的功能（包括以后新装的）开不开（Y7）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unlisted {
    /// 开：功能全开那种。
    On,
    /// 关：开发那种，只开列出来的。
    Off,
}

impl Unlisted {
    /// 文件、协议里的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            Unlisted::On => "on",
            Unlisted::Off => "off",
        }
    }
}

/// 一份预设文件读好的样子。每一格都可以没有：同名覆盖只写改了的。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PresetFile {
    /// 名字：一句字（施工 P-3 补），以前写成语言表的照样认。
    pub name: Option<Label>,
    /// 一句说明，写法同名字。
    pub summary: Option<Label>,
    /// 没列出来的功能开不开；几层都没写的照 [`Unlisted::On`]（[`PresetFile::unlisted`]）。
    pub unlisted: Option<Unlisted>,
    /// 功能的编号到开不开（施工 F-3 上）。
    pub features: BTreeMap<String, bool>,
    /// 以前的写法：软件包（或者和包同编号的功能）的编号到开不开。照认，[`PresetFile::opens_in`]。
    pub software: BTreeMap<String, bool>,
    /// 关掉的单件工具（开着的包里的）。
    pub tools_off: BTreeSet<String>,
    /// 图标（施工 P-5）：Lucide 的名字，只查过写法。界面上的事，不进 [`PresetFile::digest`]。
    pub icon: Option<String>,
}

impl PresetFile {
    /// 叠在 `lower` 上面（同名覆盖，16 第四节）：逐格盖，名字、说明写了的整格换掉，`[features]`、`[software]` 逐个键盖，关掉的
    /// 工具叠在一起。
    #[must_use]
    pub fn over(self, mut lower: PresetFile) -> PresetFile {
        lower.name = self.name.or(lower.name);
        lower.summary = self.summary.or(lower.summary);
        lower.icon = self.icon.or(lower.icon);
        lower.unlisted = self.unlisted.or(lower.unlisted);
        lower.features.extend(self.features);
        lower.software.extend(self.software);
        lower.tools_off.extend(self.tools_off);
        lower
    }

    /// 没列出来的软件开不开：几层都没写的是开。功能全开是默认，`[tools]` 里只关一两件的写法也是建在「其余都开」上的。
    pub fn unlisted(&self) -> Unlisted {
        self.unlisted.unwrap_or(Unlisted::On)
    }

    /// 包 `package` 里的功能 `feature` 开不开（施工 F-3 上）：`[features]` 写了的照写的；没写的照以前的 `[software]`，先认功能
    /// 的编号、再认包的编号；都没写的照 `unlisted`。
    pub fn opens_in(&self, feature: &str, package: &str) -> bool {
        self.features
            .get(feature)
            .or_else(|| self.software.get(feature))
            .or_else(|| self.software.get(package))
            .copied()
            .unwrap_or(self.unlisted() == Unlisted::On)
    }

    /// 编号和包一样的功能 `feature` 开不开：人格记忆、人设遵循提醒这种没写功能、整个包算一个的（[`PresetFile::opens_in`]）。
    pub fn opens(&self, feature: &str) -> bool {
        self.opens_in(feature, feature)
    }

    /// 包 `package` 里归功能 `feature` 的工具 `tool` 留不留在工具面上：功能开着，这一件也没被 `[tools]` 关掉（走查 C1）。
    pub fn keeps(&self, feature: &str, package: &str, tool: &str) -> bool {
        self.opens_in(feature, package) && !self.tools_off.contains(tool)
    }

    /// 叠好的文件的指纹（施工 P-2 下）：记进快照，回合开始时执行器照它认出预设的文件改了。名字、说明照以前的写法算
    /// （施工 P-3 补：没写的是空表、语言表照原样，一句字的是那句字），默认人格那一格照没写算（施工 P-4 上撤了）：以前造的
    /// 快照照旧对得上，开着的会话不白白换一次快照。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：几格总写得成 JSON。
    pub fn digest(&self) -> ContentHash {
        let fields = (
            label_json(self.name.as_ref()),
            label_json(self.summary.as_ref()),
            None::<String>,
            self.unlisted.map(Unlisted::as_str),
            &self.software,
            &self.tools_off,
        );
        // 没写 `[features]` 的照以前的几格算（施工 F-3 上）：以前造的快照照旧对得上。
        let bytes = match self.features.is_empty() {
            true => serde_json::to_vec(&fields),
            false => serde_json::to_vec(&(fields, &self.features)),
        };
        ContentHash::of(&bytes.expect("预设的几格写得成 JSON"))
    }
}

/// 快照里记的预设（施工 P-2 中，`Snapshot::preset`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresetPin {
    /// 预设的编号。
    pub id: String,
    /// 造会话时装了、这个预设没开的软件，照编号排。都开着的不写。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub off: Vec<String>,
    /// 叠好的文件的指纹（施工 P-2 下，[`PresetFile::digest`]）：回合开始时照它认出预设改了。P-2（中）造的没有：不换。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<ContentHash>,
}

impl PresetPin {
    /// 和 `other` 说的是同一回事（施工 F-3 上）：编号、指纹一样，没开的照现在装了的功能 `features` 读（以前记的包编号换成它的
    /// 功能，[`Features::read_legacy`]）也一样。升级以后预设没改的会话不为改了写法换一次快照、断一次缓存。
    pub fn means_the_same(&self, other: &PresetPin, features: &Features) -> bool {
        self.id == other.id
            && self.digest == other.digest
            && features.read_legacy(&self.off) == features.read_legacy(&other.off)
    }
}

/// 开会话时找好的预设（施工 P-2 中）：编号、叠好的文件，和这台机器上装了、这个预设没开的功能（照编号排；Y8 那一行照它写）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chosen {
    /// 编号。
    pub id: String,
    /// 叠好的文件。
    pub file: PresetFile,
    /// 装了、没开的功能。
    pub off: Vec<String>,
    /// 文件的指纹（施工 P-2 下）：照找到的那一份算，[`Chosen::keeping_memory`] 改了记忆那一格也不变。
    pub digest: ContentHash,
}

impl Chosen {
    /// 照装了的功能 `features` 算好没开的那几个。
    pub fn new(id: String, file: PresetFile, features: &Features) -> Chosen {
        let digest = file.digest();
        let off = off(&file, features);
        Chosen {
            id,
            file,
            off,
            digest,
        }
    }

    /// 换预设时记忆照开会话时的（施工 P-2 下，L3）：人格记忆开不开改成 `open`，没开的那几个照 `features` 重新算，指纹不变。
    #[must_use]
    pub fn keeping_memory(mut self, open: bool, features: &Features) -> Chosen {
        self.file.features.insert(MEMORY.to_string(), open);
        self.off = off(&self.file, features);
        self
    }

    /// 记进快照的那一份。
    pub fn pin(&self) -> PresetPin {
        PresetPin {
            id: self.id.clone(),
            off: self.off.clone(),
            digest: Some(self.digest.clone()),
        }
    }
}

/// 装了的功能 `features` 里 `file` 没开的，照编号排、不重复。
fn off(file: &PresetFile, features: &Features) -> Vec<String> {
    let off: BTreeSet<String> = features
        .iter()
        .filter(|feature| !file.opens_in(&feature.id, &feature.package))
        .map(|feature| feature.id.clone())
        .collect();
    off.into_iter().collect()
}

/// 名字、说明照以前的写法写成 JSON（[`PresetFile::digest`]）：没写的是空表，语言表照原样，一句字的是那句字。
fn label_json(label: Option<&Label>) -> serde_json::Value {
    match label {
        None => serde_json::json!({}),
        Some(Label::Each(phrases)) => serde_json::json!(phrases),
        Some(Label::One(text)) => serde_json::json!(text),
    }
}

#[cfg(test)]
mod tests;
