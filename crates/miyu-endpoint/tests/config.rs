//! 读配置（施工 8-2，`docs/blueprint/config.md`「协议」「守着它的」）：真核心照磁盘上的几份配置起来。握手的 `language`
//! （`auto` 照头报的系统语言，`zh`、`ja`、别的）、`config_errors`；`config.schema`、`config.get`（`cwd`、`all`、`files`、
//! `problems`）、`config.check` 的样子和报错的话；不认识的键；造会话时开局只读照配置、照信任着的项目配置。

mod support;

use serde_json::{Value, json};

use miyu_kernel::event::Body;
use miyu_session::testkit::Script;
use miyu_store::config_file::version;

use support::*;

/// 用不着模型。
fn script() -> Script {
    Script::new([])
}

/// 握手，头报的系统语言是 `locale`（没有的不报），交回回应。
async fn hello(client: &mut Client, locale: Option<&str>) -> Value {
    let mut params = json!({
        "protocol": [1, 1],
        "head": {"kind": "test", "version": "0.0.0"},
        "token": TOKEN,
    });
    if let Some(locale) = locale {
        params["locale"] = json!(locale);
    }
    client.call("hello-1", "hello", params).await
}

#[tokio::test]
async fn hello_says_the_language_by_the_setting_or_the_system() {
    let home = Home::new();
    let core = home.core_configured(&script(), None, &[]);
    for (locale, language) in [
        (Some("zh-CN"), "zh"),
        (Some("ja_JP.UTF-8"), "ja"),
        (Some("fr-FR"), "en"),
        (None, "en"),
    ] {
        let mut client = Client::connect(core.clone());
        let reply = hello(&mut client, locale).await;
        assert_eq!(reply["result"]["language"], json!(language), "{locale:?}");
        assert!(
            reply["result"].get("config_errors").is_none(),
            "没有错误不写"
        );
    }
    home.write("home/alice/settings.toml", "[ui]\nlanguage = \"en\"\n");
    let mut client = Client::connect(home.core_configured(&script(), None, &[]));
    let reply = hello(&mut client, Some("zh-CN")).await;
    assert_eq!(reply["result"]["language"], "en", "定了的照定的");
    let refused = client.call("c1", "no.such", json!({})).await;
    assert_eq!(
        refused["error"]["message"], "There is no such method.",
        "拒绝的话也照它"
    );
    home.write("home/alice/settings.toml", "[ui]\nlanguage = \"ja\"\n");
    let mut client = Client::connect(home.core_configured(&script(), None, &[]));
    assert_eq!(
        hello(&mut client, Some("zh-CN")).await["result"]["language"],
        "ja"
    );
    let refused = client.call("c1", "no.such", json!({})).await;
    assert_eq!(
        refused["error"]["message"], "There is no such method.",
        "日文的拒绝照英文"
    );
}

#[tokio::test]
async fn hello_counts_errors_but_not_warnings() {
    let home = Home::new();
    home.write("system/config.toml", "[log]\nlevel = \"verbose\"\n");
    home.write(
        "home/alice/settings.toml",
        "[ui]\nlangauge = \"zh\"\nlanguage = 3\n",
    );
    let mut client = Client::connect(home.core_configured(&script(), None, &[]));
    assert_eq!(hello(&mut client, None).await["result"]["config_errors"], 2);
}

#[tokio::test]
async fn the_schema_lists_the_items_in_the_language_of_the_connection() {
    let home = Home::new();
    let mut client = Client::connect(home.core_configured(&script(), None, &[]));
    hello(&mut client, Some("zh-CN")).await;
    let reply = client
        .call("c1", "config.schema", json!({"keys": ["ui.language"]}))
        .await;
    assert_eq!(
        reply["result"],
        json!({"groups":[{"id":"display","name":"显示","page":"general"}],"items":[{"applies":"now","common":true,"control":"select","default":"auto","description":"终端、网页、命令行给你看的字用哪种话。auto 跟着终端或浏览器的语言。","group":"display","key":"ui.language","layers":["system","personal"],"name":"界面语言","options":[{"name":"跟随系统","value":"auto"},{"name":"中文","value":"zh"},{"name":"English","value":"en"},{"name":"日本語","value":"ja"}],"page":"general","type":"option"}],"pages":[{"id":"general","name":"通用"}]}),
        "和图纸的例子一字不差"
    );
    let all = client.call("c2", "config.schema", json!({})).await;
    let keys: Vec<&str> = all["result"]["items"]
        .as_array()
        .expect("有项")
        .iter()
        .map(|item| item["key"].as_str().expect("有键"))
        .collect();
    assert_eq!(
        keys,
        [
            "ui.language",
            "ui.startup",
            "permission.start_read_only",
            "log.level"
        ]
    );
    let switch = &all["result"]["items"][2];
    assert_eq!(switch["type"], "bool");
    assert_eq!(switch["tighten"], "true_only");
    assert_eq!(switch["control"], "toggle");
    assert_eq!(switch["applies"], "new_session");
    assert!(switch.get("options").is_none());
    assert_eq!(all["result"]["items"][3]["env"], "MIYU_LOG");
    assert_eq!(
        all["result"]["pages"],
        json!([{"id":"general","name":"通用"},{"id":"permissions","name":"权限"},{"id":"advanced","name":"高级"}])
    );
}

#[tokio::test]
async fn unknown_keys_are_refused_with_the_nearest_ones() {
    let home = Home::new();
    let mut client = Client::connect(home.core_configured(&script(), None, &[]));
    hello(&mut client, Some("zh-CN")).await;
    for method in ["config.schema", "config.get"] {
        let reply = client
            .call(
                "c1",
                method,
                json!({"keys": ["ui.langauge", "ui.language", "no.such"]}),
            )
            .await;
        assert_eq!(reason(&reply), Some("unknown_config_key"), "{method}");
        assert_eq!(reply["error"]["message"], "没有这一项配置。");
        assert_eq!(
            reply["error"]["data"]["problems"],
            json!([
                {"code":"unknown_key","key":"ui.langauge","level":"error","message":"没有 ui.langauge 这一项。是不是想写 ui.language？","suggest":"ui.language"},
                {"code":"unknown_key","key":"no.such","level":"error","message":"没有 no.such 这一项。"},
            ]),
            "{method}"
        );
    }
}

#[tokio::test]
async fn get_says_every_value_and_where_it_came_from() {
    let home = Home::new();
    let system = "#:schema ../state/config/config.schema.json\n\n[log]\nlevel = \"debug\"\n\n[ui]\nlanguage = \"en\"\n";
    home.write("system/config.toml", system);
    home.write("home/alice/settings.toml", "\n\n[ui]\nlanguage = \"zh\"\n");
    let mut client = Client::connect(home.core_configured(&script(), None, &[]));
    hello(&mut client, Some("zh-CN")).await;
    let reply = client.call("c1", "config.get", json!({})).await;
    assert_eq!(
        reply["result"]["items"],
        json!({
            "log.level": {"origin": {"file": "system/config.toml", "layer": "system", "line": 4}, "value": "debug"},
            "permission.start_read_only": {"origin": {"layer": "default"}, "value": false},
            "ui.language": {"origin": {"file": "home/alice/settings.toml", "layer": "personal", "line": 4}, "value": "zh"},
            "ui.startup": {"origin": {"layer": "default"}, "value": "new"},
        })
    );
    assert_eq!(
        reply["result"]["files"],
        json!({
            "personal": {"file": "home/alice/settings.toml", "version": version(b"\n\n[ui]\nlanguage = \"zh\"\n")},
            "secrets": {"file": "system/secrets.toml"},
            "system": {"file": "system/config.toml", "version": version(system.as_bytes())},
        })
    );
    assert_eq!(reply["result"]["problems"], json!([]));
    let one = client
        .call(
            "c2",
            "config.get",
            json!({"keys": ["ui.language"], "all": true}),
        )
        .await;
    assert_eq!(
        one["result"]["items"],
        json!({"ui.language": {
            "layers": [
                {"origin": {"file": "home/alice/settings.toml", "layer": "personal", "line": 4}, "used": true, "value": "zh"},
                {"origin": {"file": "system/config.toml", "layer": "system", "line": 7}, "used": false, "value": "en"},
                {"origin": {"layer": "default"}, "used": false, "value": "auto"},
            ],
            "origin": {"file": "home/alice/settings.toml", "layer": "personal", "line": 4},
            "value": "zh",
        }})
    );
}

#[tokio::test]
async fn missing_files_have_no_version_and_the_environment_wins() {
    let home = Home::new();
    home.write("system/config.toml", "log.level = \"info\"\n");
    let core = home.core_configured(&script(), None, &[("MIYU_LOG", "trace")]);
    let mut client = Client::connect(core);
    hello(&mut client, None).await;
    let reply = client
        .call(
            "c1",
            "config.get",
            json!({"keys": ["log.level"], "all": true}),
        )
        .await;
    assert_eq!(reply["result"]["files"]["personal"]["version"], Value::Null);
    let level = &reply["result"]["items"]["log.level"];
    assert_eq!(level["origin"], json!({"layer": "env", "name": "MIYU_LOG"}));
    assert_eq!(level["value"], "trace");
    assert_eq!(level["layers"][0]["used"], true);
    assert_eq!(level["layers"][1]["origin"]["layer"], "system");
}

#[tokio::test]
async fn a_broken_item_is_dropped_and_reported_with_what_is_used() {
    let home = Home::new();
    home.write("system/config.toml", "[log]\nlevel = \"verbose\"\n");
    home.write(
        "home/alice/settings.toml",
        "[ui]\nlanguage = \"zh\"\n\n\n\n\nlangauge = \"zh\"\n[permission]\nstart_read_only = \"yes\"\n",
    );
    let mut client = Client::connect(home.core_configured(&script(), None, &[]));
    hello(&mut client, Some("zh-CN")).await;
    let reply = client.call("c1", "config.get", json!({})).await;
    assert_eq!(
        reply["result"]["items"]["ui.language"]["value"], "zh",
        "别的照用"
    );
    assert_eq!(
        reply["result"]["items"]["log.level"]["origin"],
        json!({"layer": "default"})
    );
    assert_eq!(
        reply["result"]["problems"],
        json!([
            {"code":"not_an_option","column":9,"expected":"error、warn、info、debug、trace 或 off","file":"system/config.toml","got":"\"verbose\"","key":"log.level","level":"error","line":2,
             "message":"log.level 只能是 error、warn、info、debug、trace 或 off，写的是 \"verbose\"。改成其中一个，例如 log.level = \"info\"。这一项先照 \"info\" 用着（默认值）。",
             "using":{"from":"default","value":"info"}},
            {"code":"unknown_key","column":1,"file":"home/alice/settings.toml","got":"\"zh\"","key":"ui.langauge","level":"warning","line":7,
             "message":"没有 ui.langauge 这一项。是不是想写 ui.language？这一行先不管，原样留着。","suggest":"ui.language"},
            {"code":"wrong_type","column":19,"expected":"true 或 false","file":"home/alice/settings.toml","got":"\"yes\"","key":"permission.start_read_only","level":"error","line":9,
             "message":"permission.start_read_only 要写 true 或 false，写的是 \"yes\"。改成 permission.start_read_only = true。这一项先照 false 用着（默认值）。",
             "using":{"from":"default","value":false}},
        ])
    );
}

#[tokio::test]
async fn a_file_that_cannot_be_read_is_used_as_nothing() {
    let home = Home::new();
    home.write("home/alice/settings.toml", "[ui]\nlanguage = \"zh\n");
    home.write("system/config.toml", "ui.language = \"en\"\n");
    let mut client = Client::connect(home.core_configured(&script(), None, &[]));
    hello(&mut client, None).await;
    let reply = client.call("c1", "config.get", json!({})).await;
    assert_eq!(reply["result"]["items"]["ui.language"]["value"], "en");
    let problem = &reply["result"]["problems"][0];
    assert_eq!(problem["code"], "syntax");
    assert_eq!(problem["using"], json!({"from": "nothing"}));
    assert_eq!(
        (problem["line"].clone(), problem["key"].clone()),
        (json!(2), Value::Null)
    );
    let message = problem["message"].as_str().expect("有话");
    assert!(
        message.starts_with("Not valid TOML: ")
            && message.ends_with(". The file is not used for now."),
        "{message}"
    );
}

#[tokio::test]
async fn check_looks_at_a_text_without_using_it() {
    let home = Home::new();
    home.write("system/config.toml", "permission.start_read_only = true\n");
    let mut client = Client::connect(home.core_configured(&script(), None, &[]));
    hello(&mut client, Some("en")).await;
    let check = |id: &'static str, layer: &'static str, text: &'static str| json!({"id": id, "layer": layer, "text": text});
    let mut asked = Vec::new();
    for params in [
        check("c1", "system", "[log]\nlevel = \"verbose\"\n"),
        check("c2", "personal", "[log]\nlevel = \"debug\"\n"),
        check("c3", "project", "[permission]\nstart_read_only = false\n"),
        check("c4", "personal", "ui.language = \"zh\"\n"),
    ] {
        let id = params["id"].as_str().expect("有编号").to_string();
        let reply = client
            .call(
                &id,
                "config.check",
                json!({"layer": params["layer"], "text": params["text"]}),
            )
            .await;
        asked.push(reply["result"]["problems"].clone());
    }
    let messages: Vec<Value> = asked
        .iter()
        .map(|problems| {
            json!(
                problems
                    .as_array()
                    .expect("是数组")
                    .iter()
                    .map(|p| p["message"].clone())
                    .collect::<Vec<_>>()
            )
        })
        .collect();
    assert_eq!(
        messages,
        [
            json!([
                "log.level must be error, warn, info, debug, trace or off, not \"verbose\". Write one of them, e.g. log.level = \"info\". Using \"info\" (the default) for now."
            ]),
            json!([
                "log.level belongs in the system config. It does not count in personal settings. Move it to the system config."
            ]),
            json!([
                "A project config can only make limits stricter. permission.start_read_only is true, and false here is looser, so it does not count."
            ]),
            json!([]),
        ]
    );
    assert!(asked[0][0].get("file").is_none(), "查的是一段字，不是文件");
    assert_eq!(
        (asked[2][0]["line"].clone(), asked[2][0]["column"].clone()),
        (json!(2), json!(19))
    );
    let bad = client
        .call(
            "c5",
            "config.check",
            json!({"layer": "nowhere", "text": ""}),
        )
        .await;
    assert_eq!(reason(&bad), Some("bad_params"));
    let syntax = client
        .call(
            "c6",
            "config.check",
            json!({"layer": "system", "text": "a = "}),
        )
        .await;
    assert_eq!(syntax["result"]["problems"][0]["code"], "syntax");
    assert!(syntax["result"]["problems"][0].get("using").is_none());
}

#[tokio::test]
async fn a_session_starts_read_only_when_the_setting_says_so() {
    let home = Home::new();
    let cwd = home.work.to_string_lossy().into_owned();
    let mut client = Client::connect(home.core_configured(&script(), None, &[]));
    hello(&mut client, None).await;
    let plain = client.create("c1", &cwd).await;
    home.write(
        "home/alice/settings.toml",
        "[permission]\nstart_read_only = true\n",
    );
    let mut client = Client::connect(home.core_configured(&script(), None, &[]));
    hello(&mut client, None).await;
    let strict = client.create("c2", &cwd).await;
    let read_only = |session: &str| match &home.log(session)[0].body {
        Body::SessionCreated(created) => created.permission.read_only,
        other => panic!("第一条是 session.created：{other:?}"),
    };
    assert!(!read_only(&plain), "默认不是");
    assert!(read_only(&strict), "个人设置打开了");
}
