//! 选择窗、编辑窗写到哪（蓝图 `tui.md`「配置页」第 36 到 39 条）：配置项照旧写个人设置；改预设、改人格的一个键，
//! 新建预设、新建人格（只填名字）另记，关窗时回到它的详情窗。预设、人格两边共用的几样小东西也在这里。

use serde::Deserialize;

use super::choose::{PersonaView, PresetView};
use crate::core::Refusal;
use crate::settings::popup::{Confirm, Delete, Popup};
use crate::settings::{Settings, Tone};

/// 删除、恢复出厂前问的字（人格、预设各一份）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoveTexts {
    /// 自己建的：标题 `{name}`、一行话、按钮、取消。
    pub delete: [String; 4],
    /// 改过的出厂的：标题 `{name}`、一行话、按钮（取消同上）。
    pub restore: [String; 3],
    /// 没改过的出厂的：没有能删的。
    pub nothing: String,
}

/// 选择窗、编辑窗写到哪。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Target {
    /// 个人设置里的一项。
    #[default]
    Setting,
    /// 一个预设的一个键；关窗回到它的详情窗。
    Preset(Box<PresetView>),
    /// 新建预设：填的是名字。
    NewPreset,
    /// 一个人格的一个键；关窗回到它的详情窗。
    Persona(Box<PersonaView>),
    /// 新建人格：填的是名字。
    NewPersona,
    /// 一个人格的头像：填的是图片的路径；关窗回到它的详情窗（「配置页」第 36 条）。
    Avatar(Box<PersonaView>),
    /// `model_or` 那种项（语义模型）：选了「填一个模型…」开编辑窗，别的照旧写个人设置（「配置页」第 21 条）。
    ModelOr,
}

impl Settings {
    /// 选择窗、编辑窗 `Esc`：改预设、改人格的回到它的详情窗，别的关掉。
    pub(in crate::settings) fn edit_cancel(&mut self, target: Target) {
        self.popup = match target {
            Target::Preset(view) => Some(Popup::Preset(*view)),
            Target::Persona(view) | Target::Avatar(view) => Some(Popup::Persona(*view)),
            _ => None,
        };
    }

    /// 删除、恢复出厂前先问（核心 P-3 补的 `remove`）：`delete` 是自己建的，`restore` 是改过的出厂的，没有的（没改过的
    /// 出厂的）不问、说一句没有能删的。
    pub(in crate::settings) fn confirm_remove(
        &mut self,
        name: &str,
        remove: Option<&str>,
        words: &RemoveTexts,
        ask: Delete,
    ) {
        let cancel = words.delete[3].clone();
        let (title, line, button) = match remove {
            Some("delete") => (&words.delete[0], &words.delete[1], &words.delete[2]),
            Some("restore") => (&words.restore[0], &words.restore[1], &words.restore[2]),
            _ => return self.say(words.nothing.clone(), Tone::Note),
        };
        self.popup = Some(Popup::Confirm(Confirm {
            title: title.replace("{name}", name),
            lines: vec![(line.clone(), true)],
            buttons: vec![(button.clone(), true), (cancel, false)],
            sel: 1,
            ask,
        }));
    }
}

/// 被拒了写什么：写错的（`preset_invalid`、`persona_invalid`）写照界面语言的那一句（核心 P-3 补的 `data.message`），
/// 没有的写头一处错在哪，别的写核心的原话。
pub fn said(refusal: &Refusal) -> String {
    let data = &refusal.data;
    data["message"]
        .as_str()
        .or_else(|| data["problem"].as_str())
        .map_or_else(|| refusal.message.clone(), str::to_string)
}
