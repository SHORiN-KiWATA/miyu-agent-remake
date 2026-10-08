//! 登记的全部配置清单（`docs/blueprint/config.md`「守着它的」，施工 8-1）：清单写得对（键不重复、不互为前缀、合写法，
//! 默认值过自己的校验）；中文、英文、日文里每一项都有名字、说明，选项、用到的页和组都有名字，资源里没有多出来的；
//! 照源码树的资源生成的两份 Schema、参考文件和样本 `docs/designs/samples/config/` 逐字节一样（中文、英文），日文也
//! 生成得出来。

use std::path::{Path, PathBuf};

use miyu_config::ConfigWords;
use miyu_core::settings::{FILES, Packaged, items, render};
use miyu_store::human::Human;
use miyu_store::packages::Packages;
use miyu_store::resources::ResourceRoot;

/// 出厂带的语言。
const LANGUAGES: [&str; 3] = ["zh", "en", "ja"];

/// 仓库的根。
fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// 出厂带的软件包拼好的配置项（施工 9-1 下）：真核心起来时也带着它们。
fn shipped() -> Packaged {
    let mut found = Packages::shipped(&ResourceRoot::at(repository().join("resources"))).read();
    Packaged::of(&mut found)
}

/// 这种语言的字，照源码树的资源目录读，并进出厂的包的字。
fn words(language: &str) -> Human {
    let packaged = shipped();
    Human::load(&ResourceRoot::at(repository().join("resources")), language)
        .unwrap_or_else(|error| panic!("{language} 的字读得出来：{error}"))
        .with_packages(
            packaged
                .manifests
                .iter()
                .map(|(id, manifest)| (id.as_str(), manifest)),
            language,
        )
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
            "ui.startup",
            "ui.head",
            "persona.default",
            "preset.default",
            "usage.currency",
            "permission.start_read_only",
            "external.bindings.<external>",
            "models.chat",
            "models.vision",
            "pools.<id>.models",
            "pools.<id>.strategy",
            "pools.<id>.subagent",
            "pools.<id>.description",
            "providers.<id>.name",
            "providers.<id>.driver",
            "providers.<id>.base_url",
            "providers.<id>.keys",
            "providers.<id>.catalog",
            "providers.<id>.price_multiplier",
            "providers.<id>.local",
            "providers.<id>.cache",
            "providers.<id>.models.<model>.window",
            "providers.<id>.models.<model>.catalog",
            "providers.<id>.models.<model>.max_output",
            "providers.<id>.models.<model>.inputs",
            "providers.<id>.models.<model>.tools",
            "providers.<id>.models.<model>.reasoning",
            "providers.<id>.models.<model>.effort",
            "providers.<id>.models.<model>.temperature",
            "providers.<id>.models.<model>.price_multiplier",
            "providers.<id>.models.<model>.price.input",
            "providers.<id>.models.<model>.price.output",
            "providers.<id>.models.<model>.price.cache_read",
            "providers.<id>.models.<model>.price.cache_write",
            "providers.<id>.models.<model>.price.currency",
            "models.catalog.update",
            "models.catalog.url",
            "models.catalog.every",
            "models.cooldown.rate_limited.base",
            "models.cooldown.rate_limited.max",
            "models.cooldown.retryable.base",
            "models.cooldown.retryable.max",
            "models.cooldown.auth.base",
            "models.cooldown.auth.max",
            "compaction.prepare",
            "log.level",
            "onebot.listen",
            "onebot.token"
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
        // 出厂的包的项一起查：它们用到的页（「软件包」）资源里要有名字；它们自己的名字、说明和组名在清单里，资源里没有不算。
        let packaged = shipped();
        let mut own: Vec<String> = packaged
            .items
            .iter()
            .map(|item| item.key.to_string())
            .collect();
        own.extend(
            packaged
                .manifests
                .iter()
                .map(|(id, _)| format!("组 {id}：")),
        );
        let listed: Vec<_> = items()
            .into_iter()
            .chain(packaged.items.iter().cloned())
            .collect();
        let mut problems = miyu_config::words::check(&listed, &config);
        problems.retain(|problem| !own.iter().any(|own| problem.contains(own.as_str())));
        assert_eq!(problems, Vec::<String>::new(), "{language}");
    }
}

#[test]
fn generated_files_match_the_samples_byte_for_byte() {
    for language in ["zh", "en"] {
        let rendered = render(&shipped().all(), &words(language));
        for (name, text) in FILES.into_iter().zip(rendered) {
            let text = text.unwrap_or_else(|error| panic!("{language} {name}：{error}"));
            let path = sample(name, language);
            // 样本照真核心起来时那样生成（施工 9-1 下起带出厂的包）；有意改了的，设 `MIYU_SAMPLE_WRITE=1` 跑一遍重写。
            if std::env::var_os("MIYU_SAMPLE_WRITE").is_some() {
                std::fs::write(&path, &text).expect("写得进样本");
                continue;
            }
            let expected = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{} 读不到：{error}", path.display()));
            assert_eq!(text, expected, "{language} {name} 和样本不一样");
        }
    }
}

#[test]
fn japanese_has_every_sentence_the_files_need() {
    for text in render(&shipped().all(), &words("ja")) {
        let text = text.expect("日文的字齐全");
        assert!(text.contains("表示言語"), "{text}");
    }
}

/// 施工 8-8 补：四个挡位从清单里拿掉了，写了的照不认识的键警告（原样留在文件里，`config.md` 第四条）；池的两项读得进，说明
/// 不是一行英文的 `bad_format`。
#[test]
fn tiers_are_unknown_now_and_pools_take_the_two_new_items() {
    use miyu_config::Layer;
    use miyu_config::problem::Code;
    let parse =
        |source: &str| miyu_config::parse::parse(&items(), Layer::System, source).expect("写法对");
    let parsed = parse(
        "[models.tiers]\nlite = \"a/m\"\ncheap = \"@p\"\n\n[pools.p]\nmodels = []\nsubagent = true\ndescription = \"Quick lookups.\"\n",
    );
    let problems: Vec<(Code, Option<&str>)> = parsed
        .problems
        .iter()
        .map(|problem| (problem.code, problem.key.as_deref()))
        .collect();
    assert_eq!(
        problems,
        [
            (Code::UnknownKey, Some("models.tiers.lite")),
            (Code::UnknownKey, Some("models.tiers.cheap")),
        ]
    );
    assert_eq!(
        parsed.entries["pools.p.subagent"].value,
        miyu_config::Value::Bool(true)
    );
    let parsed = parse("[pools.p]\nmodels = []\ndescription = \"快速查东西的池\"\n");
    let codes: Vec<Code> = parsed.problems.iter().map(|problem| problem.code).collect();
    assert_eq!(codes, [Code::BadFormat], "说明要写英文");
}
