//! 登记的全部配置清单（`docs/blueprint/config.md`「守着它的」，施工 8-1）：清单写得对（键不重复、不互为前缀、合写法，
//! 默认值过自己的校验）；中文、英文、日文里每一项都有名字、说明，选项、用到的页和组都有名字，资源里没有多出来的；
//! 照源码树的资源生成的两份 Schema、参考文件和样本 `docs/designs/samples/config/` 逐字节一样（中文、英文），日文也
//! 生成得出来。

use std::path::{Path, PathBuf};

use miyu_config::ConfigWords;
use miyu_core::settings::{FILES, items, render};
use miyu_store::human::Human;
use miyu_store::resources::ResourceRoot;

/// 出厂带的语言。
const LANGUAGES: [&str; 3] = ["zh", "en", "ja"];

/// 仓库的根。
fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// 这种语言的字，照源码树的资源目录读。
fn words(language: &str) -> Human {
    Human::load(&ResourceRoot::at(repository().join("resources")), language)
        .unwrap_or_else(|error| panic!("{language} 的字读得出来：{error}"))
}

/// 生成的文件 `name` 在这种语言下的样本：`config.schema.json` 的中文是 `config.schema.zh.json`。
fn sample(name: &str, language: &str) -> PathBuf {
    let (stem, extension) = name.rsplit_once('.').expect("有扩展名");
    repository()
        .join("docs/designs/samples/config")
        .join(format!("{stem}.{language}.{extension}"))
}

#[test]
fn the_registered_list_is_well_formed() {
    let items = items();
    assert_eq!(miyu_config::list::check(&items), Vec::<String>::new());
    let keys: Vec<&str> = items.iter().map(|item| item.key).collect();
    assert_eq!(
        keys,
        [
            "ui.language",
            "tui.startup",
            "permission.start_read_only",
            "models.chat",
            "providers.<id>.driver",
            "providers.<id>.base_url",
            "providers.<id>.keys",
            "providers.<id>.catalog",
            "providers.<id>.models.<model>.window",
            "log.level"
        ],
        "照登记的先后"
    );
}

#[test]
fn every_language_names_every_item_and_nothing_more() {
    for language in LANGUAGES {
        // 直接读文件：找不到的语言会退回英文，经 `Human::load` 查不出日文缺了。
        let file = repository().join(format!("resources/core/human/{language}.json"));
        let text = std::fs::read_to_string(&file).expect("读得到");
        let json: serde_json::Value = serde_json::from_str(&text).expect("是 JSON");
        let config: ConfigWords =
            serde_json::from_value(json["config"].clone()).expect("配置那一格写法对");
        assert_eq!(
            miyu_config::words::check(&items(), &config),
            Vec::<String>::new(),
            "{language}"
        );
    }
}

#[test]
fn generated_files_match_the_samples_byte_for_byte() {
    for language in ["zh", "en"] {
        let rendered = render(&items(), &words(language));
        for (name, text) in FILES.into_iter().zip(rendered) {
            let text = text.unwrap_or_else(|error| panic!("{language} {name}：{error}"));
            let path = sample(name, language);
            let expected = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{} 读不到：{error}", path.display()));
            assert_eq!(text, expected, "{language} {name} 和样本不一样");
        }
    }
}

#[test]
fn japanese_has_every_sentence_the_files_need() {
    for text in render(&items(), &words("ja")) {
        let text = text.expect("日文的字齐全");
        assert!(text.contains("表示言語"), "{text}");
    }
}
