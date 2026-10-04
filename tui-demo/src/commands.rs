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
    /// 别名：打它一样执行，筛的时候也算（`/undo` 的 `rewind`）。
    #[serde(default)]
    pub aliases: Vec<String>,
    /// 列表里名字后面那一句。
    pub summary: String,
    /// 做什么。
    pub run: Run,
    /// 名字后面空一格能接参数（`/compact 要求`，蓝图「斜杠命令列表」第 5 条）。
    #[serde(default)]
    pub args: bool,
}

/// 回车时输入框里那一行是什么（蓝图「斜杠命令列表」第 4、5 条）。
#[derive(Debug)]
pub enum Line<'a> {
    /// 一条命令，带着名字后面的字（没写的是 `None`）。
    Command(&'a Spec, Option<&'a str>),
    /// 像命令，没有这个命令：弹「命令不存在」。
    Unknown,
    /// 一句话，发给她。
    Talk,
}

/// 命令做什么。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Run {
    /// 撤销上一轮（`session.revert`）。
    Revert,
    /// 恢复刚才的撤销（`session.unrevert`）。
    Unrevert,
    /// 这一轮原样重来一遍（`session.redo`，蓝图「斜杠命令」`/redo`）。
    Redo,
    /// 编辑上一句：放进输入框改，回车照改过的重来（蓝图「输入框」第 13 条）。
    Edit,
    /// 现在就压缩上下文（`session.compact`），后面的字是给摘要的要求。
    Compact,
    /// 要一段回顾（`session.recap`，蓝图「回顾」）。
    Recap,
    /// 改名、去掉标题（`session.set_meta`，蓝图「改名」）。
    Rename,
    /// 会话列表，切过去（蓝图「会话列表 `/sessions`」）。
    Sessions,
    /// 换这个会话的模型（蓝图「配置与模型」第 1 条）。
    Model,
    /// 换正在用的模型的思考强度（蓝图「配置与模型」思考强度）。
    Effort,
    /// 开新会话：清界面回首页，第一句话时再开（蓝图「斜杠命令」`/new`）。
    New,
    /// 清空上下文，会话不变（`session.clear`，蓝图「正文」第 9 条）。
    Clear,
    /// 复制她上一轮的回答（蓝图「斜杠命令」`/copy`）。
    Copy,
    /// 换下一套主题。
    Theme,
    /// 换下一套图标（蓝图「图标」）。
    Icons,
    /// 退出程序。
    Quit,
    /// 帮助：输入框上面开一个框，列命令和按键（蓝图「帮助 `/help`」）。
    Help,
    /// 在中英文之间切换界面语言（蓝图「界面语言」）。
    Language,
    /// 演示用的假命令：还不做事，只说一句。
    Fake,
    /// 演示：推一份假的待办。
    DemoTodo,
    /// 演示：她问一个假的问题（蓝图「确认和提问的抽屉」）。
    DemoAsk,
    /// 演示：要一次假的权限确认。
    DemoApprove,
}

impl Commands {
    /// 照打的字筛：名字（或别名）以它开头的排前面，含着它的排后面，各自照清单的先后。
    pub fn filter(&self, typed: &str) -> Vec<&Spec> {
        let typed = typed.to_lowercase();
        let (mut starts, mut contains) = (Vec::new(), Vec::new());
        for spec in &self.commands {
            let mut names = std::iter::once(&spec.name).chain(&spec.aliases);
            if names.clone().any(|n| n.starts_with(&typed)) {
                starts.push(spec);
            } else if names.any(|n| n.contains(&typed)) {
                contains.push(spec);
            }
        }
        starts.extend(contains);
        starts
    }

    /// 名字或别名正好是 `name` 的那一条。
    pub fn find(&self, name: &str) -> Option<&Spec> {
        self.commands
            .iter()
            .find(|s| s.name == name || s.aliases.iter().any(|a| a == name))
    }
}

impl Commands {
    /// 回车时这一行是什么：`/名字` 是命令；能带参数的命令后面空一格接着的字是参数；别的后面跟了字的是一句话。
    pub fn read<'a>(&'a self, text: &'a str) -> Line<'a> {
        let Some(name) = typed(text) else {
            return Line::Talk;
        };
        // 名字后面的字：`typed` 认过的名字就在 `/` 后面。
        let words = text[1 + name.len()..].trim();
        match (self.find(name), words.is_empty()) {
            (Some(spec), true) => Line::Command(spec, None),
            (Some(spec), false) if spec.args => Line::Command(spec, Some(words)),
            (None, true) => Line::Unknown,
            _ => Line::Talk,
        }
    }
}

/// 命令列表要不要开、照什么筛：像命令名，名字后面还没打空格（打了空格是在写参数，名字已经打全了）。
pub fn menu_typed(text: &str) -> Option<&str> {
    let name = typed(text)?;
    let after = &text[1 + name.len()..];
    after.is_empty().then_some(name)
}

/// 输入框里的字要不要当命令：`/` 开头，后面像个命令名（英文字母打头，只有英文字母、数字、`-`、`_`；
/// 刚打一个 `/` 也算）。`/你吃了吗`、路径这些不算（`tui.md`「斜杠命令列表」第 1、4 条）。交回命令名。
pub fn typed(text: &str) -> Option<&str> {
    let rest = text.strip_prefix('/')?;
    // 名字紧挨着 `/`：`/ compact` 这种的名字是空的。
    let name = rest.split(char::is_whitespace).next().unwrap_or_default();
    let starts = name.chars().next().is_none_or(|c| c.is_ascii_alphabetic());
    let rest_ok = name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'));
    (starts && rest_ok).then_some(name)
}

#[cfg(test)]
mod tests {
    use super::{Commands, Line, menu_typed, typed};

    #[test]
    fn a_command_that_takes_words_keeps_them() {
        let c = commands();
        match c.read("/compact 重点保留 数据库设计") {
            Line::Command(spec, Some(words)) => {
                assert_eq!(spec.name, "compact");
                assert_eq!(words, "重点保留 数据库设计");
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(c.read("/compact"), Line::Command(s, None) if s.name == "compact"));
        assert!(
            matches!(c.read("/compact   "), Line::Command(_, None)),
            "只有空白：没写"
        );
        assert!(matches!(c.read("/undo"), Line::Command(s, None) if s.name == "undo"));
        assert!(
            matches!(c.read("/rewind"), Line::Command(s, None) if s.name == "undo"),
            "别名"
        );
        assert!(
            matches!(c.read("/theme 深色"), Line::Talk),
            "不带参数的命令后面跟了字：一句话"
        );
        assert!(
            matches!(c.read("/etc 目录是干什么的"), Line::Talk),
            "不弹命令不存在"
        );
        assert!(matches!(c.read("/nosuch"), Line::Unknown));
        assert!(matches!(c.read("你好"), Line::Talk));
        assert!(matches!(c.read("/你吃了吗"), Line::Talk));
        assert!(matches!(c.read("/ compact"), Line::Talk), "名字要紧挨着 /");
    }

    #[test]
    fn typing_words_after_the_name_closes_the_list() {
        assert_eq!(menu_typed("/comp"), Some("comp"));
        assert_eq!(menu_typed("/"), Some(""));
        assert_eq!(menu_typed("/compact "), None, "打了空格：名字打全了");
        assert_eq!(menu_typed("/compact 重点"), None);
        assert_eq!(menu_typed("/你吃了吗"), None);
        assert_eq!(menu_typed("/ compact"), None, "名字要紧挨着 /");
    }
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
        assert_eq!(
            names[..3],
            ["edit", "effort", "exit"],
            "开头对得上的在前，照登记的先后"
        );
        assert!(names.contains(&"level"), "含着 e 的也在：{names:?}");
    }

    #[test]
    fn clear_can_also_be_typed_as_reset() {
        // 2026-09-30 项目主人：/clear 别名 /reset，会话不变、清空上下文。
        let commands = commands();
        assert_eq!(
            commands.find("clear").map(|s| s.run),
            Some(super::Run::Clear)
        );
        assert_eq!(
            commands.find("reset").map(|s| s.run),
            Some(super::Run::Clear)
        );
    }

    #[test]
    fn undo_and_restore_follow_the_core_names() {
        // 2026-09-29 跟 main 的命令改名：恢复叫 /restore，撤销也可以打 /rewind。/redo 09-30 回来了，换成核心的
        // 重做（施工 4-7 再补），不是恢复。
        let commands = commands();
        assert_eq!(
            commands.find("restore").map(|s| s.run),
            Some(super::Run::Unrevert)
        );
        assert_eq!(
            commands.find("rewind").map(|s| s.run),
            Some(super::Run::Revert)
        );
        assert_eq!(commands.find("redo").map(|s| s.run), Some(super::Run::Redo));
        let names: Vec<_> = commands
            .filter("rew")
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        assert_eq!(names, ["undo"], "筛的时候别名也算，列表里写正名");
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
