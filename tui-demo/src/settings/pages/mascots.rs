//! 「吉祥物」那一项的选择窗列的吉祥物包（蓝图 `tui.md`「吉祥物包」第 3 条）：照 `package.list` 挑读成了的 `kind = "mascot"`，
//! 和人格一个形状（编号、名字），第一行「内置」由 `choices` 加。

use serde_json::Value;

use super::personas::Persona;

/// 读 `package.list` 的回应：吉祥物包一行行。
pub fn read(got: &Value) -> Vec<Persona> {
    let text = |v: &Value| v.as_str().map(str::to_string);
    got["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|p| p["kind"] == "mascot")
        .map(|p| Persona {
            id: text(&p["package"]).unwrap_or_default(),
            name: text(&p["name"]),
            summary: text(&p["summary"]),
            problem: text(&p["problem"]),
            avatar: None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::read;

    #[test]
    fn only_mascot_packages_are_listed() {
        let got = json!({"packages": [
            {"package": "tui", "kind": "ui", "name": "终端界面"},
            {"package": "pudding", "kind": "mascot", "name": "布丁", "mascot": {"model": "mascot.json"}},
            {"package": "broken", "layer": "personal", "code": "bad_toml", "problem": "写错了"}
        ]});
        let list = read(&got);
        assert_eq!(list.len(), 1);
        assert_eq!((list[0].id.as_str(), list[0].label()), ("pudding", "布丁"));
    }
}
