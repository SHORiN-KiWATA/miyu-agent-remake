//! 人格（核心 P-1、P-3，蓝图「配置页」第 35 到 37 条）：`persona.list` 一行行，`persona.get` 一个人格的详情。只读人
//! 用得上的几样：名字、说明、人设和角色扮演提示写没写、示范对话几轮。

use serde_json::Value;

pub use crate::core::{Persona, read_personas as read};

/// `persona.get` 读来的一个人格的详情。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Detail {
    /// 编号：存的时候用，不给人看。
    pub id: String,
    /// 名字。
    pub name: Option<String>,
    /// 说明。
    pub summary: Option<String>,
    /// 写了人设。
    pub prompt: bool,
    /// 写了角色扮演提示。
    pub reminders: bool,
    /// 示范对话几轮。
    pub examples: u64,
    /// 能不能删（核心 P-3 补）：`delete` 自己建的，`restore` 改过的出厂的，`None` 没改过的出厂的。
    pub remove: Option<String>,
    /// 头像的版本（核心 P-5）；没有头像的是 `None`。
    pub avatar: Option<String>,
}

/// 读 `persona.get` 的回应。`prompts` 里的两样是写没写（核心 P-3 补）；以前给的是来处（写了的是字），一样认成写了。
pub fn detail(got: &Value) -> Detail {
    let text = |v: &Value| v.as_str().map(str::to_string);
    let written = |v: &Value| v == &Value::Bool(true) || v.is_string();
    Detail {
        id: text(&got["persona"]).unwrap_or_default(),
        name: text(&got["name"]),
        summary: text(&got["summary"]),
        prompt: written(&got["prompts"]["persona"]),
        reminders: written(&got["prompts"]["reminders"]),
        examples: got["examples"].as_u64().unwrap_or_default(),
        remove: text(&got["remove"]),
        avatar: text(&got["avatar"]),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{detail, read};

    #[test]
    fn the_list_and_one_persona_read_as_the_core_sends_them() {
        let got = read(&json!({"personas": [
            {"persona": "engineer", "name": "软件工程师", "summary": null},
            {"persona": "broken", "problem": "home persona.toml:2: unknown key"}
        ]}));
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].label(), "软件工程师");
        assert!(got[0].problem.is_none());
        assert_eq!(got[1].label(), "broken", "没名字的写编号");
        assert!(got[1].problem.is_some());
        let one = detail(&json!({"persona": "persona-1", "name": "美羽",
            "prompts": {"persona": true, "reminders": false}, "examples": 2}));
        assert_eq!(one.name.as_deref(), Some("美羽"));
        assert!(one.prompt && !one.reminders);
        assert_eq!(one.examples, 2);
    }
}
