//! 人格头像替换吉祥物（蓝图 `tui.md`「空会话的首页」第 10 条，2026-10-11 项目主人）：首页画这个新会话要用的人格的头像
//! （人格框开着的跟着光标），侧边栏画开着的会话的人格的头像；没有头像、「无人格」、终端显示不了图的照旧画吉祥物。
//! 头像照 `persona.list` 给的版本去要、存成文件（`avatars.rs`），每一帧以后挑一张还没存的去要。

use std::path::PathBuf;

use serde_json::json;

use super::new_session::{Kind, Pick};
use super::{App, Panel};
use crate::core::{Command, Update};
use crate::oobe::asker::TAG_BASE;

/// 要头像的请求编号。
const AVATAR_TAG: u64 = TAG_BASE - 5;

/// 画在哪。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// 空会话的首页。
    Home,
    /// 宽屏的侧边栏。
    Sidebar,
}

/// 这一处画什么。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Avatar {
    /// 没有头像：照开关画吉祥物。
    None,
    /// 有头像，还没存下来：先空着（不先画吉祥物再换）。
    Waiting,
    /// 有头像，存在这份文件里。
    File(PathBuf),
}

/// 首页画谁的头像：人格框开着的照光标停的那一行（`under`），选过的照选的，测具设的照设的，开了会话的照会话的，都没有的
/// 照默认的。「无人格」（编号空着）没有头像。
fn home_persona<'a>(
    under: Option<&'a str>,
    pick: &'a Pick,
    session: Option<&'a str>,
) -> Option<&'a str> {
    under
        .or(pick.chosen.as_deref())
        .or(pick.env.as_deref())
        .or(session)
        .or(pick.default.as_deref())
        .filter(|id| !id.is_empty())
}

/// 侧边栏画谁的头像：开着的会话的人格（侧边栏只在开了会话以后有）。
fn sidebar_persona(session: Option<&str>) -> Option<&str> {
    session.filter(|id| !id.is_empty())
}

impl App {
    /// 这一处的人格和它的头像的版本；没有头像的是 `None`。
    fn wanted(&self, place: Place) -> Option<(&str, &str)> {
        let session = self.transcript.persona.as_deref();
        let persona = match place {
            Place::Home => {
                let under = match self.panel {
                    Some(Panel::Pick {
                        kind: Kind::Persona,
                        selected,
                    }) => Some(
                        self.pick_list(Kind::Persona)
                            .0
                            .get(selected)
                            .map_or("", |p| p.id.as_str()),
                    ),
                    _ => None,
                };
                home_persona(under, &self.persona, session)
            }
            Place::Sidebar => sidebar_persona(session),
        }?;
        let row = self
            .pick_list(Kind::Persona)
            .0
            .iter()
            .find(|p| p.id == persona)?;
        Some((persona, row.avatar.as_deref()?))
    }

    /// 这一处画什么。终端显示不了图的、要不到的当没有头像。
    pub fn avatar(&self, place: Place) -> Avatar {
        if !self.figures.borrow().shows() {
            return Avatar::None;
        }
        let Some((_, version)) = self.wanted(place) else {
            return Avatar::None;
        };
        match self.avatars.file(version) {
            Some(path) => Avatar::File(path),
            None if self.avatars.refused(version) => Avatar::None,
            None => Avatar::Waiting,
        }
    }

    /// 挑一张还没存的头像去要（`tick`）。
    pub(super) fn ask_avatars(&mut self) {
        if !self.figures.borrow().shows() {
            return;
        }
        let wanted: Vec<(String, String)> = [Place::Home, Place::Sidebar]
            .into_iter()
            .filter_map(|place| self.wanted(place))
            .map(|(p, v)| (p.to_string(), v.to_string()))
            .collect();
        let refs: Vec<(&str, &str)> = wanted
            .iter()
            .map(|(p, v)| (p.as_str(), v.as_str()))
            .collect();
        if let Some(persona) = self.avatars.next(&refs) {
            self.core.send(Command::Ask {
                tag: AVATAR_TAG,
                method: "persona.avatar",
                params: json!({ "persona": persona }),
            });
        }
    }

    /// 头像回来了（交回 `true`）；断开了的，在要的那张不等了，连上以后再要。
    pub(super) fn avatar_update(&mut self, update: &Update) -> bool {
        match update {
            Update::Answer { tag, result } if *tag == AVATAR_TAG => {
                self.avatars.answered(result.as_ref().ok());
                true
            }
            Update::Disconnected => {
                self.avatars.dropped();
                false
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Pick, home_persona, sidebar_persona};

    fn pick(chosen: Option<&str>, default: Option<&str>) -> Pick {
        Pick {
            chosen: chosen.map(str::to_string),
            default: default.map(str::to_string),
            ..Pick::default()
        }
    }

    #[test]
    fn the_home_follows_the_persona_box_then_the_choice_then_the_default() {
        let none = pick(None, Some("miyu"));
        assert_eq!(
            home_persona(Some("cat"), &none, None),
            Some("cat"),
            "框开着跟着光标"
        );
        assert_eq!(
            home_persona(Some(""), &none, None),
            None,
            "光标在「无人格」上"
        );
        assert_eq!(
            home_persona(None, &none, None),
            Some("miyu"),
            "没选过的照默认的"
        );
        let chosen = pick(Some("cat"), Some("miyu"));
        assert_eq!(
            home_persona(None, &chosen, None),
            Some("cat"),
            "选过的照选的"
        );
        let nobody = pick(Some(""), Some("miyu"));
        assert_eq!(home_persona(None, &nobody, None), None, "选了「无人格」");
        assert_eq!(
            home_persona(None, &pick(None, None), Some("dog")),
            Some("dog")
        );
    }

    #[test]
    fn the_sidebar_follows_the_open_session() {
        assert_eq!(sidebar_persona(Some("dog")), Some("dog"));
        assert_eq!(sidebar_persona(None), None, "没带人格的会话不照新会话选的");
    }
}
