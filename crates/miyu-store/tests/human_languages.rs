//! 给人看的字的每一种语言（施工 4-5 补）：内核和每个软件包都有中文、英文、日文三份，键、工具的样子、每一句
//! 要的字段和英文那一份一样。找不到的语言会退回英文，所以这里直接查文件，不经 `Human::load`。

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use miyu_kernel::event::Said;
use miyu_kernel::template::Template;
use miyu_store::human::Human;
use miyu_store::resources::ResourceRoot;
use serde_json::Value;

/// 出厂带的语言。
const LANGUAGES: [&str; 3] = ["zh", "en", "ja"];

/// 源码树里的资源目录。
fn resources() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources")
}

/// 有 `human/` 的几处：内核，和每个软件包。
fn places() -> Vec<PathBuf> {
    let mut places = vec![resources().join("core")];
    let mut packages: Vec<PathBuf> = std::fs::read_dir(resources().join("software"))
        .expect("有软件包")
        .map(|entry| entry.expect("读得了").path())
        .filter(|path| path.join("human").is_dir())
        .collect();
    packages.sort();
    places.extend(packages);
    places
}

fn read(place: &Path, language: &str) -> Value {
    let file = place.join("human").join(format!("{language}.json"));
    let text = std::fs::read_to_string(&file)
        .unwrap_or_else(|error| panic!("{} 读不到：{error}", file.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("{} 不是 JSON：{error}", file.display()))
}

/// 每一句要的字段。
fn said(value: &Value) -> BTreeMap<String, BTreeSet<String>> {
    value
        .get("said")
        .and_then(Value::as_object)
        .map(|said| {
            said.iter()
                .map(|(key, source)| {
                    let template =
                        Template::parse(source.as_str().expect("是字符串")).expect("写法对");
                    let fields = template.fields().into_iter().map(str::to_owned).collect();
                    (key.clone(), fields)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// 每件工具除了显示名以外的样子（跟哪个参数、符号、下面那一块）。
fn tools(value: &Value) -> BTreeMap<String, Value> {
    value
        .get("tools")
        .and_then(Value::as_object)
        .map(|tools| {
            tools
                .iter()
                .map(|(name, face)| {
                    let mut face = face.clone();
                    let name_field = face.as_object_mut().expect("是对象").remove("name");
                    assert!(
                        name_field
                            .and_then(|name| name.as_str().map(str::to_owned))
                            .is_some_and(|name| !name.is_empty()),
                        "{name} 没有显示名"
                    );
                    (name.clone(), face)
                })
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn every_language_has_the_same_sentences_and_tools_as_english() {
    for place in places() {
        let english = read(&place, "en");
        for language in LANGUAGES {
            let words = read(&place, language);
            assert_eq!(
                said(&words),
                said(&english),
                "{} 的 {language}：说法的键、字段和英文的不一样",
                place.display()
            );
            assert_eq!(
                tools(&words),
                tools(&english),
                "{} 的 {language}：工具和英文的不一样",
                place.display()
            );
        }
    }
}

#[test]
fn japanese_turns_into_japanese() {
    let words = Human::load(&ResourceRoot::at(resources()), "ja").expect("读得出来");
    let lines = Said::new("software/basesystem/read/lines").with("count", "37");
    assert_eq!(words.say(&lines).as_deref(), Some("37 行"));
    assert_eq!(
        words.tool("read").map(|face| face.name.as_str()),
        Some("読み取り")
    );
    assert_eq!(
        words
            .say(&Said::new("core/tool-results/cancelled-before"))
            .as_deref(),
        Some("中断しました：実行していません")
    );
}
