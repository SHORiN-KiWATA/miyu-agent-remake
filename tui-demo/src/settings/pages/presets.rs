//! 预设（核心 P-2、P-3、F-3 下，蓝图「配置页」第 38、39 条）：`preset.get` 一个预设的详情，只读人用得上的几样：名字、
//! 一个个功能和归它的工具（`crate::features`）。列表和人格一个形状（`core::read_presets`）。

use serde_json::Value;

use crate::features::{self, Feature};

/// `preset.get` 读来的一个预设的详情。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Detail {
    /// 编号：存的时候用，不给人看。
    pub id: String,
    /// 名字。
    pub name: Option<String>,
    /// 说明。
    pub summary: Option<String>,
    /// 一个个功能，照核心给的先后（核心 F-3 下起 `features`，原来的 `software`、`tools` 撤了）。
    pub features: Vec<Feature>,
    /// 能不能删（核心 P-3 补）：`delete` 自己建的，`restore` 改过的出厂的，`None` 没改过的出厂的。
    pub remove: Option<String>,
}

/// 读 `preset.get` 的回应。
pub fn detail(got: &Value) -> Detail {
    let text = |v: &Value| v.as_str().map(str::to_string);
    Detail {
        id: text(&got["preset"]).unwrap_or_default(),
        name: text(&got["name"]),
        summary: text(&got["summary"]),
        features: features::read(got),
        remove: text(&got["remove"]),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::detail;

    #[test]
    fn a_preset_reads_its_features_and_their_tools() {
        let got = detail(&json!({"preset": "dev", "name": "开发", "remove": "delete",
            "features": [
                {"id": "files", "name": "文件读写", "on": true, "installed": true,
                 "tools": [{"name": "grep", "label": "搜内容", "on": false}]},
                {"id": "web", "name": "联网", "on": true, "installed": false, "tools": []}]}));
        assert_eq!(got.name.as_deref(), Some("开发"));
        assert_eq!(got.features[0].name, "文件读写");
        assert_eq!(got.features[0].tools[0].label, "搜内容");
        assert!(!got.features[0].tools[0].on);
        assert!(!got.features[1].installed, "写了没装的");
        assert_eq!(got.remove.as_deref(), Some("delete"));
    }
}
