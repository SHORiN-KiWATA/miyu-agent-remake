//! 给人看的字照仓库里的资源读：内核那一份的 `said`，各软件包的工具的样子，照 `human.rs` 的编号规矩。

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::Value;

use miyu_kernel::event::Said;
use miyu_view::{Face, Kinds, Texts, Words};

/// 仓库的资源目录。
fn resources() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources")
}

fn read(path: PathBuf) -> Value {
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).expect("资源是 JSON")
}

/// 一种语言的字。
pub struct ResourceWords {
    said: BTreeMap<String, String>,
    faces: BTreeMap<String, Face>,
}

impl ResourceWords {
    fn load(language: &str) -> ResourceWords {
        let core = read(resources().join(format!("core/human/{language}.json")));
        let said = core["said"]
            .as_object()
            .expect("有 said")
            .iter()
            .map(|(k, v)| {
                (
                    format!("core/{k}"),
                    v.as_str().unwrap_or_default().to_string(),
                )
            })
            .collect();
        let mut faces = BTreeMap::new();
        for package in ["basesystem", "net", "memory"] {
            let path = resources().join(format!("software/{package}/human/{language}.json"));
            if !path.exists() {
                continue;
            }
            for (name, face) in read(path)["tools"].as_object().into_iter().flatten() {
                faces.insert(
                    name.clone(),
                    Face {
                        name: face["name"].as_str().unwrap_or_default().to_string(),
                        subject: face["subject"].as_str().map(str::to_string),
                    },
                );
            }
        }
        ResourceWords { said, faces }
    }
}

impl Words for ResourceWords {
    fn face(&self, name: &str) -> Option<Face> {
        self.faces.get(name).cloned()
    }

    fn say(&self, said: &Said) -> Option<String> {
        // 中文、日文没写单数那一句的照抄不带 `/one` 的（`human.rs`）。
        let template = self
            .said
            .get(&said.key)
            .or_else(|| self.said.get(said.key.strip_suffix("/one")?))?;
        let mut out = template.clone();
        for (field, value) in &said.fields {
            out = out.replace(&format!("{{{field}}}"), value);
        }
        Some(out)
    }
}

/// 连接是 `language` 的那一套字，另带英文，工具的分类照 `core/view.json`，家目录是 `/home/alice`。
pub fn texts(language: &str) -> Texts {
    let kinds: Kinds =
        serde_json::from_value(read(resources().join("core/view.json"))).expect("view.json 写法对");
    Texts {
        local: Box::new(ResourceWords::load(language)),
        english: Box::new(ResourceWords::load("en")),
        kinds,
        home: Some("/home/alice".to_string()),
    }
}
