//! `config.schema` 读成几页（蓝图「配置页」第 5、32 条）：页、组、一项项。通用、权限、高级这几页照它画；键里有人起
//! 的名字的（`providers.<id>.…`）归「供应商和模型」，别的头专有的（`web.*`）不列。

use serde_json::Value;

/// 一项。
/// 下拉的一个选项（`config.schema` 的 `options`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opt {
    /// 值。
    pub value: Value,
    /// 给人看的名字。
    pub name: String,
    /// 核心查得出的一句，接在名字后面暗色写（核心 R-5 再补：`models.embedding` 的 `local` 写内置模型的名字）。
    pub note: Option<String>,
    /// 用得了：核心查得出用不了的（没装内置语义模型的包）列着、灰的、选不了（核心 R-5 三补）。
    pub available: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// 键。
    pub key: String,
    /// 控件：`select`、`toggle`、`number`、`text`、`list`、`secret`、`custom:…`。
    pub control: String,
    /// 下拉的选项。
    pub options: Vec<Opt>,
    /// 能写在哪几层。
    pub layers: Vec<String>,
    /// 默认值；没有的是 `null`。
    pub default: Value,
    /// 名字。
    pub name: String,
    /// 在哪一页。
    pub page: String,
    /// 哪一组。
    pub group: String,
    /// 常用项：排在组里前面。
    pub common: bool,
}

impl Item {
    /// 能写进个人设置。
    pub fn personal(&self) -> bool {
        self.layers.iter().any(|l| l == "personal")
    }

    /// 只能写系统配置（运行日志级别这类，「配置页」第 33 条）。
    pub fn system_only(&self) -> bool {
        !self.personal() && self.layers.iter().any(|l| l == "system")
    }

    /// 终端改得了：个人设置、系统配置有一层能写。
    pub fn writable(&self) -> bool {
        self.personal() || self.system_only()
    }
}

/// 一组。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    /// 编号。
    pub id: String,
    /// 名字。
    pub name: String,
    /// 在哪一页。
    pub page: String,
}

/// 读来的清单。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Schema {
    /// 页：编号、名字，照核心交回的先后。
    pub pages: Vec<(String, String)>,
    /// 组，照核心交回的先后。
    pub groups: Vec<Group>,
    /// 项，照清单登记的先后。
    pub items: Vec<Item>,
}

/// 一页里的一行。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// 组名：第几组。
    Group(usize),
    /// 一项：第几项。
    Item(usize),
}

/// 「供应商和模型」那一页的编号：不另列。
pub const MODELS: &str = "models";
/// 「通用」那一页的编号。
pub const GENERAL: &str = "general";
/// 终端自己的配置项的键打头的那一截（终端软件包的编号）。
const OWN: &str = "tui.";

/// 终端自己的配置项里另分一组的：键的开头、组的编号（「配置页」第 33 条，2026-10-11 项目主人：时间线单独一个标题，
/// 吉祥物也分出去）。
const OWN_GROUPS: [(&str, &str); 2] = [
    ("tui.timeline_", "tui.timeline"),
    ("tui.mascot", "tui.mascot"),
];

/// 另分的那几组的名字（`text/zh.json` 的 `settings.more.own_groups`）。
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnGroups {
    /// 时间线。
    pub timeline: String,
    /// 吉祥物。
    pub mascot: String,
}

impl OwnGroups {
    /// 照组的编号取名字。
    fn name(&self, id: &str) -> &str {
        if id == "tui.timeline" {
            &self.timeline
        } else {
            &self.mascot
        }
    }
}

impl Schema {
    /// 读 `config.schema` 的回应。
    pub fn read(got: &Value) -> Schema {
        let text = |v: &Value| v.as_str().unwrap_or_default().to_string();
        let pages = got["pages"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|p| (text(&p["id"]), text(&p["name"])))
            .collect();
        let groups = got["groups"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|g| Group {
                id: text(&g["id"]),
                name: text(&g["name"]),
                page: text(&g["page"]),
            })
            .collect();
        let items = got["items"]
            .as_array()
            .into_iter()
            .flatten()
            // 标了 `hidden` 的设置页不画（照样能写、能查，核心 9-1 下）。
            .filter(|i| i["hidden"] != true)
            .map(|i| Item {
                key: text(&i["key"]),
                control: text(&i["control"]),
                options: i["options"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|o| Opt {
                        value: o["value"].clone(),
                        name: text(&o["name"]),
                        note: o["note"].as_str().map(str::to_string),
                        available: o["available"] != false,
                    })
                    .collect(),
                layers: i["layers"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(text)
                    .collect(),
                default: i["default"].clone(),
                name: text(&i["name"]),
                page: text(&i["page"]),
                group: text(&i["group"]),
                common: i["common"] == true,
            })
            // 键里有人起的名字的归供应商和模型；别的头专有的不列（蓝图「配置页」第 5 条）。
            .filter(|i: &Item| !i.key.contains('<') && !i.key.starts_with("web."))
            .collect();
        let mut schema = Schema {
            pages,
            groups,
            items,
        };
        schema.own_items_to_general();
        schema
    }

    /// 终端自己的配置项（`tui.` 开头，核心排在「软件包」页这个包那一组）挪到「通用」页，组照旧（「配置页」第 33 条，
    /// 2026-10-10 项目主人）。
    fn own_items_to_general(&mut self) {
        let mut moved = Vec::new();
        for item in self.items.iter_mut().filter(|i| i.key.starts_with(OWN)) {
            item.page = GENERAL.to_string();
            moved.push(item.group.clone());
        }
        for group in self.groups.iter_mut().filter(|g| moved.contains(&g.id)) {
            group.page = GENERAL.to_string();
        }
    }

    /// 终端自己的配置项里，时间线、吉祥物各分一组，排在终端那一组后面、同一页；组名照界面的字。核心没有这几项的（旧核心）
    /// 那几组是空的，不画。
    pub fn split_own(&mut self, names: &OwnGroups) {
        let own = |g: &Group, items: &[Item]| {
            items
                .iter()
                .any(|i| i.key.starts_with(OWN) && i.group == g.id)
        };
        let Some(at) = self.groups.iter().position(|g| own(g, &self.items)) else {
            return;
        };
        let page = self.groups[at].page.clone();
        for (n, (prefix, id)) in OWN_GROUPS.iter().enumerate() {
            for item in self.items.iter_mut().filter(|i| i.key.starts_with(prefix)) {
                item.group = (*id).to_string();
            }
            let group = Group {
                id: (*id).to_string(),
                name: names.name(id).to_string(),
                page: page.clone(),
            };
            self.groups.insert(at + 1 + n, group);
        }
    }

    /// 这一页的名字。
    pub fn page_name(&self, page: &str) -> Option<&str> {
        self.pages
            .iter()
            .find(|(id, _)| id == page)
            .map(|(_, name)| name.as_str())
    }

    /// 这一页有项的组名，照组的先后（主菜单底下那一行写它们）。
    pub fn group_names(&self, page: &str) -> Vec<&str> {
        self.groups
            .iter()
            .filter(|g| g.page == page)
            .filter(|g| self.items.iter().any(|i| i.page == page && i.group == g.id))
            .map(|g| g.name.as_str())
            .collect()
    }

    /// 一页排成行：组名一行，下面它的项，常用的在前；一项都没有的组不写。
    pub fn rows(&self, page: &str) -> Vec<Row> {
        let mut out = Vec::new();
        for (g, group) in self
            .groups
            .iter()
            .enumerate()
            .filter(|(_, g)| g.page == page)
        {
            let mut items: Vec<usize> = (0..self.items.len())
                .filter(|&i| self.items[i].page == page && self.items[i].group == group.id)
                .collect();
            if items.is_empty() {
                continue;
            }
            items.sort_by_key(|&i| !self.items[i].common);
            out.push(Row::Group(g));
            out.extend(items.into_iter().map(Row::Item));
        }
        out
    }

    /// 有项的页，照先后，不算「供应商和模型」那一页。
    pub fn listed_pages(&self) -> Vec<&str> {
        self.pages
            .iter()
            .map(|(id, _)| id.as_str())
            .filter(|id| *id != MODELS && !self.rows(id).is_empty())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Row, Schema};

    fn sample() -> Schema {
        Schema::read(&json!({
            "pages": [{"id": "general", "name": "通用"}, {"id": "permissions", "name": "权限"}, {"id": "models", "name": "模型"}],
            "groups": [{"id": "display", "name": "显示", "page": "general"}, {"id": "persona", "name": "人格", "page": "general"},
                       {"id": "sessions", "name": "会话", "page": "permissions"}, {"id": "empty", "name": "空", "page": "general"},
                       {"id": "providers", "name": "供应商", "page": "models"}],
            "items": [
                {"key": "ui.startup", "control": "select", "layers": ["system", "personal"], "name": "启动时", "page": "general", "group": "display",
                 "options": [{"value": "new", "name": "新会话"}, {"value": "recent", "name": "最近的"}], "default": "new"},
                {"key": "ui.language", "control": "select", "layers": ["system", "personal"], "name": "界面语言", "page": "general", "group": "display", "common": true},
                {"key": "persona.default", "control": "select", "layers": ["system", "personal"], "name": "默认人格", "page": "general", "group": "persona"},
                {"key": "permission.start_read_only", "control": "toggle", "layers": ["system"], "name": "只读开始", "page": "permissions", "group": "sessions"},
                {"key": "providers.<id>.base_url", "control": "text", "layers": ["personal"], "name": "地址", "page": "models", "group": "providers"},
                {"key": "web.theme", "control": "select", "layers": ["personal"], "name": "网页主题", "page": "general", "group": "display"},
                {"key": "ui.ticket", "control": "text", "layers": ["system"], "name": "票据", "page": "general", "group": "display", "hidden": true}
            ]
        }))
    }

    #[test]
    fn pages_list_their_groups_with_common_items_first_and_models_stay_out() {
        let s = sample();
        assert_eq!(s.items.len(), 4, "人起的名字的、web 的、hidden 的不要");
        assert_eq!(
            s.rows("general"),
            vec![
                Row::Group(0),
                Row::Item(1),
                Row::Item(0),
                Row::Group(1),
                Row::Item(2)
            ]
        );
        assert_eq!(s.group_names("general"), ["显示", "人格"], "空的组不写");
        assert_eq!(
            s.listed_pages(),
            ["general", "permissions"],
            "模型那一页不另列"
        );
        assert!(!s.items[3].personal(), "只能写系统配置");
        assert_eq!(s.page_name("permissions"), Some("权限"));
    }

    #[test]
    fn the_terminals_own_items_sit_on_the_general_page() {
        // 「配置页」第 33 条（2026-10-10 项目主人：时间线的配置项应在通用里、又是终端自己的）。
        let s = Schema::read(&json!({
            "pages": [{"id": "general", "name": "通用"}, {"id": "packages", "name": "软件包"}],
            "groups": [{"id": "display", "name": "显示", "page": "general"},
                       {"id": "onebot", "name": "接入QQ", "page": "packages"},
                       {"id": "tui", "name": "终端界面", "page": "packages"}],
            "items": [
                {"key": "ui.language", "control": "select", "layers": ["personal"], "name": "界面语言", "page": "general", "group": "display"},
                {"key": "onebot.port", "control": "number", "layers": ["system"], "name": "端口", "page": "packages", "group": "onebot"},
                {"key": "tui.timeline_fold", "control": "toggle", "layers": ["system", "personal"], "name": "做完收起", "page": "packages", "group": "tui"}
            ]
        }));
        assert_eq!(s.group_names("general"), ["显示", "终端界面"]);
        assert_eq!(s.group_names("packages"), ["接入QQ"]);
    }

    #[test]
    fn the_terminals_timeline_and_mascot_items_get_their_own_groups() {
        // 「配置页」第 33 条（2026-10-11 项目主人：时间线相关的配置项单独一个标题，不和终端界面混在一起；吉祥物也分出去）。
        let item = |key: &str, name: &str| json!({"key": key, "control": "toggle", "layers": ["personal"], "name": name, "page": "packages", "group": "tui"});
        let mut s = Schema::read(&json!({
            "pages": [{"id": "general", "name": "通用"}, {"id": "packages", "name": "软件包"}],
            "groups": [{"id": "display", "name": "显示", "page": "general"},
                       {"id": "tui", "name": "终端界面", "page": "packages"}],
            "items": [
                {"key": "ui.language", "control": "select", "layers": ["personal"], "name": "界面语言", "page": "general", "group": "display"},
                item("tui.icons", "图标"),
                item("tui.timeline_fold", "回复完成后折叠时间线"),
                item("tui.mascot_home", "首页显示吉祥物"),
                item("tui.mascot", "吉祥物")
            ]
        }));
        s.split_own(&super::OwnGroups {
            timeline: "时间线".into(),
            mascot: "吉祥物".into(),
        });
        assert_eq!(
            s.group_names("general"),
            ["显示", "终端界面", "时间线", "吉祥物"]
        );
        let group = |key: &str| {
            s.items
                .iter()
                .find(|i| i.key == key)
                .map(|i| i.group.clone())
        };
        assert_eq!(group("tui.icons").as_deref(), Some("tui"));
        assert_eq!(group("tui.timeline_fold").as_deref(), Some("tui.timeline"));
        assert_eq!(group("tui.mascot").as_deref(), Some("tui.mascot"));
    }
}
