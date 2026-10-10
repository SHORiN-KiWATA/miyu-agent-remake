//! 新会话先选人格、再选预设（蓝图 `tui.md`「新会话：人格、工作区」第 1 到 3 条；2026-10-07 项目主人定每次开新会话
//! 都先弹框选，2026-10-08 定先人格、再预设，两个分开的框）：起来进空会话、`/new` 以后，在输入框上面开框，选了才能
//! 打字；光标停在上一次选的（最近那个会话用的，2026-10-09 项目主人：「照旧弹框，光标停在上一次的」）上，没有的停在默认
//! 的上，`Enter` 选，`Esc` 照光标起初停的那一个（用不了时不关、弹一句：开会话一定要有预设，核心 Y12）。
//! 选的记着，开会话时带上；开了会话不能换。测具设了 `MIYU_TUI_PERSONA`、`MIYU_TUI_PRESET` 的照它、不弹框（人格写 `-` 是
//! 无人格）。

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::{App, Panel};
use crate::core::{Command, Persona};

/// 新会话要选的哪一样。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// 人格：她是谁。
    Persona,
    /// 预设：这次怎么工作。
    Preset,
}

/// 照这个先后问（2026-10-08 项目主人定先人格、再预设）。
const ORDER: [Kind; 2] = [Kind::Persona, Kind::Preset];

/// 一样要选的：读来的一行行（人格和预设一个形状）、默认的、这个新会话选了的、测具设的。
#[derive(Debug, Default)]
pub struct Pick {
    /// 核心交回的；还没读到的是 `None`，读不了（旧核心）的是空的。
    pub list: Option<Vec<Persona>>,
    /// 默认的编号（`persona.default`、`preset.default`）。
    pub default: Option<String>,
    /// 这个新会话选了的；还没选的是 `None`。
    pub chosen: Option<String>,
    /// 测具设的（空的不算）。
    pub env: Option<String>,
    /// 框开着时光标起初停的那一行：会话列表晚到了，人还没挪过的照它改停的地方。
    pub opened: Option<usize>,
}

impl Pick {
    /// 照环境变量 `name` 起一份（测具设了的照它、不弹框）。
    pub fn from_env(name: &str) -> Self {
        Self {
            // `-` 是明着不带（「无人格」，编号空着）。
            env: std::env::var(name)
                .ok()
                .filter(|v| !v.is_empty())
                .map(|v| if v == "-" { String::new() } else { v }),
            ..Self::default()
        }
    }

    /// 默认的在第几行；没有、用不了的是 `None`。
    fn default_index(&self) -> Option<usize> {
        let default = self.default.as_deref()?;
        let list = self.list.as_ref()?;
        list.iter()
            .position(|p| p.id == default && p.problem.is_none())
    }

    /// 框起初停在第几行：上一次选的（`last`：最近那个会话用的，`Some(None)` 是它没带）列表里有、用得了的停它，不然停
    /// 默认的，再不然停第一个用得了的。人格框第一行「无人格」的编号是空的，没带人格的停它。
    pub(super) fn start(&self, last: Option<Option<&str>>) -> usize {
        let list = self.list.as_deref().unwrap_or_default();
        self.preferred(last)
            .or_else(|| usable(list).next())
            .unwrap_or(0)
    }

    /// 上一次选的、默认的，用得了的那一行；都没有的是 `None`（`Esc` 照它：预设都用不了时不关框，核心 Y12）。
    fn preferred(&self, last: Option<Option<&str>>) -> Option<usize> {
        let list = self.list.as_deref().unwrap_or_default();
        let found = |id: &str| list.iter().position(|p| p.id == id && p.problem.is_none());
        last.and_then(|id| found(id.unwrap_or_default()))
            .or_else(|| self.default_index())
    }

    /// 编号写成名字（照列表，没有的写编号）。
    fn name<'a>(&'a self, id: &'a str) -> &'a str {
        self.list
            .iter()
            .flatten()
            .find(|p| p.id == id)
            .map_or(id, |p| p.label())
    }
}

impl App {
    fn pick(&self, kind: Kind) -> &Pick {
        match kind {
            Kind::Persona => &self.persona,
            Kind::Preset => &self.preset,
        }
    }

    fn pick_mut(&mut self, kind: Kind) -> &mut Pick {
        match kind {
            Kind::Persona => &mut self.persona,
            Kind::Preset => &mut self.preset,
        }
    }

    /// 核心交回了人格、预设：记下，该问的时候开框。人格最前面加一行「无人格」（编号空着，开会话时发 `"persona": null`，
    /// 2026-10-08 项目主人：人格允许为空）；核心不认人格的（列表是空的）不加、不问。
    pub(super) fn listed(&mut self, kind: Kind, mut list: Vec<Persona>) {
        if kind == Kind::Persona && !list.is_empty() {
            let texts = &self.config.text.persona_box;
            list.insert(
                0,
                Persona {
                    id: String::new(),
                    name: Some(texts.none.clone()),
                    summary: Some(texts.none_note.clone()),
                    problem: None,
                    avatar: None,
                },
            );
        }
        self.pick_mut(kind).list = Some(list);
        self.ask_next();
    }

    /// 该问的时候开框：还没开会话、没开着别的框；照先后，没选的先问。测具设了的直接用；核心不认的（列表是空的）
    /// 不问；还没读到的等读到了再问。
    pub(super) fn ask_next(&mut self) {
        if !self.not_opened() || self.panel.is_some() {
            return;
        }
        for kind in ORDER {
            let pick = self.pick(kind);
            if pick.chosen.is_some() {
                continue;
            }
            if let Some(id) = pick.env.clone() {
                self.choose(kind, id);
                continue;
            }
            let Some(list) = pick.list.as_ref() else {
                return;
            };
            if list.is_empty() {
                continue;
            }
            let selected = pick.start(self.last_pick(kind));
            self.pick_mut(kind).opened = Some(selected);
            self.panel = Some(Panel::Pick { kind, selected });
            return;
        }
    }

    /// 上一次选的：最近那个会话用的（`Some(None)` 是它没带）；会话列表还没到、一个会话都没有的是 `None`。
    fn last_pick(&self, kind: Kind) -> Option<Option<&str>> {
        let latest = crate::core::latest_session(self.sessions_seen.as_deref()?)?;
        Some(match kind {
            Kind::Persona => latest.persona.as_deref(),
            Kind::Preset => latest.preset.as_deref(),
        })
    }

    /// 会话列表到了、变了：框开着、人还没挪过光标的，照上一次选的重新停。
    pub(super) fn repoint_pick(&mut self) {
        let Some(Panel::Pick { kind, selected }) = self.panel else {
            return;
        };
        if self.pick(kind).opened != Some(selected) {
            return;
        }
        let selected = self.pick(kind).start(self.last_pick(kind));
        self.pick_mut(kind).opened = Some(selected);
        self.panel = Some(Panel::Pick { kind, selected });
    }

    /// 框开着时的按键：上下挪（跳过写错的），`Enter` 选，`Esc` 照光标起初停的那一个。别的键不管：选了才能打字。
    pub(super) fn pick_key(&mut self, kind: Kind, selected: usize, key: KeyEvent) {
        let Some(list) = self.pick(kind).list.clone() else {
            self.panel = None;
            return;
        };
        let step = |down: bool| {
            let next = if down {
                usable(&list).find(|&i| i > selected)
            } else {
                usable(&list).filter(|&i| i < selected).last()
            };
            next.unwrap_or(selected)
        };
        match key.code {
            KeyCode::Down => {
                self.panel = Some(Panel::Pick {
                    kind,
                    selected: step(true),
                })
            }
            KeyCode::Up => {
                self.panel = Some(Panel::Pick {
                    kind,
                    selected: step(false),
                })
            }
            KeyCode::Enter => {
                if let Some(p) = list.get(selected).filter(|p| p.problem.is_none()) {
                    self.panel = None;
                    self.choose(kind, p.id.clone());
                    self.ask_next();
                }
            }
            // 照光标起初停的那一个（上一次选的、默认的）；人格都用不了的就是无人格（第一行），不卡住（2026-10-08 项目主人：
            // 人格允许为空）。
            KeyCode::Esc => match self
                .pick(kind)
                .preferred(self.last_pick(kind))
                .or((kind == Kind::Persona).then_some(0))
            {
                Some(i) => {
                    self.panel = None;
                    self.choose(kind, list[i].id.clone());
                    self.ask_next();
                }
                // 只有预设会到这里：开会话一定要有预设（核心 Y12）。
                None => {
                    let note = self.config.text.persona_box.preset_need.clone();
                    self.hint(note, false);
                }
            },
            _ => {}
        }
    }

    /// 选了：记着，告诉核心开会话时带上。
    fn choose(&mut self, kind: Kind, id: String) {
        let command = match kind {
            Kind::Persona => Command::Persona(id.clone()),
            Kind::Preset => Command::Preset(id.clone()),
        };
        self.core.send(command);
        self.pick_mut(kind).chosen = Some(id);
    }

    /// 第一次引导刚走完：还没开会话的照引导里写成默认的人格、预设开这一个会话，开着的框收掉，不再问（蓝图「第一次
    /// 打开的引导」第 11 条，2026-10-09 项目主人定）。已经开了会话的不动；`/new` 以后照常问。
    pub(super) fn adopt_picks(&mut self, persona: Option<String>, preset: Option<String>) {
        if !self.not_opened() {
            return;
        }
        if matches!(self.panel, Some(Panel::Pick { .. })) {
            self.panel = None;
        }
        for (kind, id) in [(Kind::Persona, persona), (Kind::Preset, preset)] {
            if let Some(id) = id {
                self.choose(kind, id);
            }
        }
        self.ask_next();
    }

    /// `/persona`、`/preset`：还没开会话的再开框换；开了的弹一句不能换。
    pub(super) fn pick_command(&mut self, kind: Kind) {
        if !self.not_opened() {
            let texts = &self.config.text.persona_box;
            let note = match kind {
                Kind::Persona => texts.locked.clone(),
                Kind::Preset => texts.preset_locked.clone(),
            };
            self.hint(note, false);
            return;
        }
        let pick = self.pick(kind);
        let Some(list) = pick.list.as_ref().filter(|l| !l.is_empty()) else {
            return;
        };
        let at = pick
            .chosen
            .as_deref()
            .and_then(|id| list.iter().position(|p| p.id == id))
            .or_else(|| pick.default_index())
            .unwrap_or(0);
        self.panel = Some(Panel::Pick { kind, selected: at });
    }

    /// 新会话：选的都清掉，再问（`/new`）。
    pub(super) fn forget_picks(&mut self) {
        self.persona.chosen = None;
        self.preset.chosen = None;
        self.ask_next();
        self.core.send(Command::ListPersonas);
        self.core.send(Command::ListPresets);
    }

    /// 开着的会话用的人格、预设，各一句（侧边栏写，订阅回应里的，核心 P-1 下、P-2 上）。
    pub fn session_picks(&self) -> Vec<String> {
        self.words(
            self.transcript.persona.as_deref(),
            self.transcript.preset.as_deref(),
        )
    }

    fn words(&self, persona: Option<&str>, preset: Option<&str>) -> Vec<String> {
        let texts = &self.config.text.persona_box;
        let mut out = Vec::new();
        if let Some(id) = persona {
            out.push(texts.chosen.replace("{name}", self.persona.name(id)));
        }
        if let Some(id) = preset {
            out.push(texts.preset_chosen.replace("{name}", self.preset.name(id)));
        }
        out
    }

    /// 框里画的：哪一样的一行行、默认的编号。
    pub fn pick_list(&self, kind: Kind) -> (&[Persona], Option<&str>) {
        let pick = self.pick(kind);
        (
            pick.list.as_deref().unwrap_or_default(),
            pick.default.as_deref(),
        )
    }

    /// 连上、配置变了：默认的照配置，两样都再读一次。
    pub(super) fn picks_configured(&mut self, persona: Option<String>, preset: Option<String>) {
        self.persona.default = persona;
        self.preset.default = preset;
        self.core.send(Command::ListPersonas);
        self.core.send(Command::ListPresets);
    }
}

/// 选得了的那几行。
fn usable(list: &[Persona]) -> impl Iterator<Item = usize> + '_ {
    (0..list.len()).filter(|&i| list[i].problem.is_none())
}

#[cfg(test)]
mod tests {
    use super::Pick;
    use crate::core::Persona;

    fn row(id: &str, problem: Option<&str>) -> Persona {
        Persona {
            id: id.into(),
            name: None,
            summary: None,
            problem: problem.map(str::to_string),
            avatar: None,
        }
    }

    #[test]
    fn the_box_starts_on_the_last_pick_then_the_default() {
        // 2026-10-09 项目主人：「照旧弹框，光标停在上一次的」。
        let pick = Pick {
            list: Some(vec![
                row("", None),
                row("miyu", None),
                row("broken", Some("写错了")),
                row("engineer", None),
            ]),
            default: Some("miyu".into()),
            ..Pick::default()
        };
        assert_eq!(pick.start(Some(Some("engineer"))), 3, "上一次选的");
        assert_eq!(pick.start(Some(None)), 0, "上一次没带人格：无人格");
        assert_eq!(
            pick.start(Some(Some("broken"))),
            1,
            "上一次的用不了：默认的"
        );
        assert_eq!(pick.start(Some(Some("gone"))), 1, "上一次的没了：默认的");
        assert_eq!(pick.start(None), 1, "没有会话：默认的");
    }
}
