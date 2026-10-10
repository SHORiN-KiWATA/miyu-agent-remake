//! `/effort` 要的（蓝图 `tui.md`「配置与模型」思考强度，核心 8-18 补）：`model.list` 里每个模型有哪几级
//! （`facts.reasoning`）、配置的默认是哪一级、写哪个配置键（`facts.effort` 的 `value`、`key`），默认的聊天模型、哪些池是
//! 轮换的（落到轮换的池上没有固定的模型可改）；顺带每个模型收哪几种附件（`abilities.rs`）。

use serde_json::Value;

/// 一个模型的思考强度。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Efforts {
    /// 引用，`<供应商>/<模型>`。
    pub reference: String,
    /// 显示名（没有的是模型名）。
    pub name: String,
    /// 有哪几级，照核心给的先后。
    pub levels: Vec<String>,
    /// 配置的默认是哪一级；没配的是 `None`（供应商那头定）。
    pub current: Option<String>,
    /// 写哪个配置键（照抄，引号也在里面）。
    pub key: String,
}

/// `model.list` 回应里 `/effort` 要的几样。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EffortList {
    /// 每个模型，有配置键的才算。
    pub models: Vec<Efforts>,
    /// 默认的聊天模型的引用（`uses.chat`）。
    pub chat: Option<String>,
    /// 轮换的池（`@名字`）。
    pub rotating: Vec<String>,
    /// 每个模型、池收哪几种附件（「输入框」第 12 条「照模型收」）。
    pub abilities: super::Abilities,
}

impl EffortList {
    /// 从 `model.list` 的回应里读；读不懂的格当没有。
    pub fn read(result: &Value) -> Self {
        let text = |v: &Value| v.as_str().map(str::to_string);
        let models = result["providers"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|p| p["models"].as_array().into_iter().flatten())
            .filter_map(|m| {
                let facts = &m["facts"];
                Some(Efforts {
                    reference: text(&m["ref"])?,
                    name: text(&facts["name"]["value"])
                        .or_else(|| text(&m["model"]))
                        .unwrap_or_default(),
                    levels: facts["reasoning"]["value"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(&text)
                        .collect(),
                    current: text(&facts["effort"]["value"]),
                    key: text(&facts["effort"]["key"])?,
                })
            })
            .collect();
        let rotating = result["pools"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|p| p["strategy"] == "rotate")
            .filter_map(|p| p["name"].as_str().map(|n| format!("@{n}")))
            .collect();
        Self {
            models,
            chat: text(&result["uses"]["chat"]),
            rotating,
            abilities: super::Abilities::read(result),
        }
    }

    /// 改哪个模型：`in_use` 是正在请求的那个（推来的 `endpoint/model`），`session` 是会话的引用；都没有的照默认的聊天
    /// 模型。用的是池的（钉住的、轮换的都算）不让改，是 `None`（2026-10-02 项目主人定）。正在请求的那个列表里找不到的，
    /// 退回会话的引用、默认的聊天模型。没开会话的不看 `in_use`：那是底栏上一次写的默认模型，默认换了要跟着换（2026-10-09
    /// 项目主人报：引导里换了模型，空会话的底栏还是旧的）。
    pub fn target(&self, in_use: Option<&str>, session: Option<&str>) -> Option<&Efforts> {
        let in_use = in_use.filter(|_| session.is_some());
        let reference = session.or(self.chat.as_deref())?;
        if reference.starts_with('@') {
            return None;
        }
        let find = |r: &str| self.models.iter().find(|m| m.reference == r);
        in_use.and_then(find).or_else(|| find(reference))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::EffortList;

    fn sample() -> EffortList {
        EffortList::read(&json!({
            "uses": {"chat": "dev/alpha"},
            "pools": [{"name": "duo", "strategy": "pin", "models": ["dev/alpha", "dev/beta"]},
                      {"name": "spin", "strategy": "rotate", "models": ["dev/alpha", "dev/beta"]}],
            "providers": [{"id": "dev", "models": [
                {"model": "alpha", "ref": "dev/alpha", "facts": {
                    "name": {"value": "Alpha"},
                    "reasoning": {"value": ["off", "low", "high"]},
                    "effort": {"value": "high", "from": "personal", "key": "providers.dev.models.alpha.effort"}}},
                {"model": "beta", "ref": "dev/beta", "facts": {
                    "reasoning": {"value": []},
                    "effort": {"value": null, "from": "default", "key": "providers.dev.models.beta.effort"}}}]}]}))
    }

    #[test]
    fn levels_default_and_key_come_from_the_model_list() {
        let list = sample();
        let alpha = &list.models[0];
        assert_eq!(alpha.name, "Alpha");
        assert_eq!(alpha.levels, ["off", "low", "high"]);
        assert_eq!(alpha.current.as_deref(), Some("high"));
        assert_eq!(alpha.key, "providers.dev.models.alpha.effort");
        assert_eq!(list.models[1].name, "beta", "没有显示名的写模型名");
        assert_eq!(list.models[1].current, None);
    }

    #[test]
    fn the_model_to_change_is_the_one_in_use_and_a_pool_has_none() {
        let list = sample();
        let target = |in_use, session| list.target(in_use, session).map(|m| m.reference.clone());
        assert_eq!(
            target(None, None).as_deref(),
            Some("dev/alpha"),
            "没开会话照默认的聊天模型"
        );
        assert_eq!(
            target(Some("dev/beta"), Some("dev/beta")).as_deref(),
            Some("dev/beta")
        );
        assert_eq!(
            target(Some("dev/beta"), Some("@duo")),
            None,
            "钉住的池也不让改"
        );
        assert_eq!(
            target(Some("dev/beta"), Some("@spin")),
            None,
            "轮换的池没有固定的模型"
        );
        assert_eq!(
            target(Some("other/x"), Some("dev/beta")).as_deref(),
            Some("dev/beta"),
            "正在请求的列表里没有：退回会话的引用"
        );
        // 2026-10-09 项目主人报：引导里换了默认模型，空会话的底栏还写着旧的。没开会话时底栏上的是上一次写的默认模型，
        // 不是正在请求的，不能拿它挑。
        assert_eq!(
            target(Some("dev/beta"), None).as_deref(),
            Some("dev/alpha"),
            "没开会话照新的默认模型"
        );
    }
}
