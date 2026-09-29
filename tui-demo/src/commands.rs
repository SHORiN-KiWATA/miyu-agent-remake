//! 斜杠命令：清单住在 `resources/commands.json`，一条一个名字、一句说明、做什么。
//!
//! 加命令只登记：写进清单，`run` 选下面几种之一；要新的一种才动代码。

use serde::Deserialize;

/// 全部斜杠命令，照清单里的先后。
#[derive(Debug, Clone, Deserialize)]
pub struct Commands {
    /// 一条条。
    pub commands: Vec<Spec>,
}

/// 一条命令。
#[derive(Debug, Clone, Deserialize)]
pub struct Spec {
    /// 名字，不带 `/`。
    pub name: String,
    /// 列表里名字后面那一句。
    pub summary: String,
    /// 做什么。
    pub run: Run,
}

/// 命令做什么。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Run {
    /// 撤销上一轮（`session.revert`）。
    Revert,
    /// 恢复刚才的撤销（`session.unrevert`）。
    Unrevert,
    /// 换下一套主题。
    Theme,
    /// 退出程序。
    Quit,
    /// 演示用的假命令：还不做事，只说一句。
    Fake,
    /// 演示：起一条假的后台命令（蓝图「后台命令、子代理和侧边栏」）。
    DemoShell,
    /// 演示：派一个假的子代理。
    DemoAgent,
    /// 演示：推一份假的待办。
    DemoTodo,
    /// 演示：她问一个假的问题（蓝图「确认和提问的抽屉」）。
    DemoAsk,
    /// 演示：要一次假的权限确认。
    DemoApprove,
}

impl Commands {
    /// 照打的字筛：名字以它开头的排前面，名字里含着它的排后面，各自照清单的先后。
    pub fn filter(&self, typed: &str) -> Vec<&Spec> {
        let typed = typed.to_lowercase();
        let (mut starts, mut contains) = (Vec::new(), Vec::new());
        for spec in &self.commands {
            if spec.name.starts_with(&typed) {
                starts.push(spec);
            } else if spec.name.contains(&typed) {
                contains.push(spec);
            }
        }
        starts.extend(contains);
        starts
    }

    /// 名字正好是 `name` 的那一条。
    pub fn find(&self, name: &str) -> Option<&Spec> {
        self.commands.iter().find(|s| s.name == name)
    }
}

/// 输入框里的字要不要当命令：`/` 开头，后面像个命令名（英文字母打头，只有英文字母、数字、`-`、`_`；
/// 刚打一个 `/` 也算）。`/你吃了吗`、路径这些不算（`tui.md`「斜杠命令列表」第 1、4 条）。交回命令名。
pub fn typed(text: &str) -> Option<&str> {
    let rest = text.strip_prefix('/')?;
    let name = rest.split_whitespace().next().unwrap_or_default();
    let starts = name.chars().next().is_none_or(|c| c.is_ascii_alphabetic());
    let rest_ok = name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'));
    (starts && rest_ok).then_some(name)
}

#[cfg(test)]
mod tests {
    use super::{Commands, typed};
    use crate::config::Config;

    fn commands() -> Commands {
        Config::builtin().unwrap().commands
    }

    #[test]
    fn prefix_matches_come_first() {
        let commands = commands();
        let names: Vec<_> = commands
            .filter("e")
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        assert_eq!(names.first(), Some(&"exit"));
        assert!(names.contains(&"level"), "含着 e 的也在：{names:?}");
    }

    #[test]
    fn paths_are_not_commands() {
        assert_eq!(typed("/undo"), Some("undo"));
        assert_eq!(typed("/"), Some(""));
        assert_eq!(typed("/home/me/a.txt"), None);
        assert_eq!(typed("undo"), None);
        // 不像命令名的不算命令，照普通的话发（`tui.md`「斜杠命令列表」第 1、4 条）。
        assert_eq!(typed("/你吃了吗"), None);
        assert_eq!(typed("/1st"), None);
        assert_eq!(typed("/foo-bar_2"), Some("foo-bar_2"));
        assert_eq!(typed("/foo 带参数"), Some("foo"));
    }
}
