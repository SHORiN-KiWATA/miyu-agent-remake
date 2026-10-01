//! 测试用：照仓库的资源目录读一种语言的字，转成界面用的那份（和核心经 `human.get` 交出来的一样）。

use miyu_store::resources::ResourceRoot;
use serde_json::json;

use super::Human;

impl Human {
    /// 照仓库 `resources/` 读 `language` 那一种。
    pub fn from_resources(language: &str) -> Human {
        let root = ResourceRoot::at(concat!(env!("CARGO_MANIFEST_DIR"), "/../resources"));
        let stored = miyu_store::human::Human::load(&root, language).expect("资源读得懂");
        let said: serde_json::Map<String, serde_json::Value> = stored
            .said_entries()
            .map(|(key, source)| (key.to_string(), json!(source)))
            .collect();
        Human::from_reply(&json!({"language": language, "said": said, "tools": stored.tools()}))
    }
}
