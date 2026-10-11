//! 卸就是清干净（施工 F-8 中下补，设计 `31-软件包.md` 第三节第 3 条、定了的 H）：真核心走一遍。卸掉的包，系统配置、个人设置
//! 里它的几项删掉，别的项、注释不动；状态目录整个删掉。家目录里装的、出厂的都一样。

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::*;

/// 一份带配置项的界面包：`xcfg.port` 两层都能写。
const XCFG: &str = "[package]\nprotocol = [1, 1]\nname = { en = \"Cfg\" }\n\n[ui]\n\n[settings.port]\ntype = \"int\"\ndefault = 8400\nlayers = [\"system\", \"personal\"]\nname = { en = \"Port\" }\n";

async fn call(client: &mut Client, method: &str, params: Value) -> Value {
    client.call("c", method, params).await
}

fn read(home: &Home, relative: &str) -> String {
    std::fs::read_to_string(home.root.path().join(relative)).unwrap_or_default()
}

#[tokio::test]
async fn removing_a_package_forgets_its_settings_and_state() {
    let home = Home::new();
    home.write("home/alice/packages/xcfg/package.toml", XCFG);
    home.write(
        "system/config.toml",
        "# 留着\nlog.level = \"debug\"\n\n[xcfg]\nport = 9000\n",
    );
    home.write("home/alice/settings.toml", "[xcfg]\nport = 9001\n");
    let mut client = Client::connect(packaged::core(&home));
    client.hello().await;
    let got = call(&mut client, "config.get", json!({"keys": ["xcfg.port"]})).await;
    assert_eq!(got["result"]["items"]["xcfg.port"]["value"], 9001, "{got}");
    let state = home.root.path().join("state/packages/xcfg");
    std::fs::create_dir_all(&state).expect("建得了");
    std::fs::write(state.join("data"), "x").expect("写得进");
    let removed = call(&mut client, "package.remove", json!({"package": "xcfg"})).await;
    assert_eq!(removed["result"]["removed"], true, "{removed}");
    let system = read(&home, "system/config.toml");
    assert!(
        !system.contains("port") && !system.contains("[xcfg]"),
        "系统配置里那一项删了，空了的表头也删了：{system}"
    );
    assert!(
        system.contains("# 留着") && system.contains("log.level = \"debug\""),
        "别的不动：{system}"
    );
    let personal = read(&home, "home/alice/settings.toml");
    assert!(!personal.contains("port"), "个人设置里的也删了：{personal}");
    assert!(!state.exists(), "状态目录删了");
}

#[tokio::test]
async fn removing_a_shipped_package_forgets_its_settings_too() {
    let home = Home::new();
    home.write(
        "system/config.toml",
        "[onebot]\nlisten = 9999\n\n[log]\nlevel = \"debug\"\n",
    );
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let state = home.root.path().join("state/packages/onebot");
    std::fs::create_dir_all(&state).expect("建得了");
    let removed = call(&mut client, "package.remove", json!({"package": "onebot"})).await;
    assert_eq!(removed["result"]["removed"], true, "{removed}");
    let system = read(&home, "system/config.toml");
    assert!(!system.contains("listen"), "{system}");
    assert!(system.contains("level = \"debug\""), "{system}");
    assert!(!state.exists());
}
