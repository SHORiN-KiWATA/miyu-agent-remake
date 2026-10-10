//! 模型收哪几种附件（蓝图 `tui.md`「输入框」第 12 条「照模型收」，核心 `model.list` 每个模型的 `facts.inputs`）：池里有一个成员
//! 收不了就算收不了（同核心，`models.md` 第十三条第 1 条）；图片例外，配了看图模型（`uses.vision`）的核心会替它看。认不出的
//! （列表里没有、没写 `inputs` 的）当收得了，不拦。

use std::collections::HashMap;

use serde_json::Value;

/// `model.list` 里头要的这一样。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Abilities {
    inputs: HashMap<String, Vec<String>>,
    pools: HashMap<String, Vec<String>>,
    vision: bool,
}

impl Abilities {
    /// 从 `model.list` 的回应里读。
    pub fn read(result: &Value) -> Self {
        let strings = |v: &Value| -> Vec<String> {
            v.as_array()
                .into_iter()
                .flatten()
                .filter_map(|s| s.as_str().map(str::to_string))
                .collect()
        };
        let inputs = result["providers"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|p| p["models"].as_array().into_iter().flatten())
            .filter_map(|m| {
                let list = m["facts"]["inputs"]["value"].as_array()?;
                let reference = m["ref"].as_str()?.to_string();
                Some((reference, strings(&Value::Array(list.clone()))))
            })
            .collect();
        let pools = result["pools"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| Some((format!("@{}", p["name"].as_str()?), strings(&p["models"]))))
            .collect();
        Self {
            inputs,
            pools,
            vision: result["uses"]["vision"].is_string(),
        }
    }

    /// `reference`（模型或 `@池`）收不收 `kind`（`image`、`pdf`、`audio`、`video`）。
    pub fn takes(&self, reference: &str, kind: &str) -> bool {
        if kind == "image" && self.vision {
            return true;
        }
        let model = |r: &str| {
            self.inputs
                .get(r)
                .is_none_or(|i| i.iter().any(|k| k == kind))
        };
        match self.pools.get(reference) {
            Some(members) => members.iter().all(|m| model(m)),
            None => model(reference),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::Abilities;

    fn list(vision: bool) -> serde_json::Value {
        json!({
            "uses": {"chat": "dev/blind", "vision": if vision { json!("dev/eyes") } else { json!(null) }},
            "pools": [
                {"name": "mixed", "models": ["dev/blind", "dev/eyes"]},
                {"name": "seeing", "models": ["dev/eyes"]}],
            "providers": [{"id": "dev", "models": [
                {"model": "blind", "ref": "dev/blind", "facts": {"inputs": {"value": ["text", "pdf"]}}},
                {"model": "eyes", "ref": "dev/eyes", "facts": {"inputs": {"value": ["text", "image"]}}},
                {"model": "plain", "ref": "dev/plain", "facts": {}}]}]})
    }

    #[test]
    fn a_model_takes_what_its_inputs_list_and_pools_take_only_what_every_member_takes() {
        let a = Abilities::read(&list(false));
        assert!(!a.takes("dev/blind", "image"));
        assert!(a.takes("dev/blind", "pdf"));
        assert!(a.takes("dev/eyes", "image"));
        assert!(!a.takes("@mixed", "image"), "池里有一个收不了就算收不了");
        assert!(!a.takes("@mixed", "pdf"));
        assert!(a.takes("@seeing", "image"));
        assert!(a.takes("dev/plain", "video"), "没写 inputs 的不拦");
        assert!(a.takes("dev/unknown", "video"), "列表里没有的不拦");
    }

    #[test]
    fn a_vision_model_lets_blind_models_take_images() {
        let a = Abilities::read(&list(true));
        assert!(a.takes("dev/blind", "image"), "核心替它看图");
        assert!(a.takes("@mixed", "image"));
        assert!(!a.takes("dev/blind", "audio"), "只有图片例外");
    }
}
