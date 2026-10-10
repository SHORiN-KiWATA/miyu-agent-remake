//! 装、卸软件包（施工 F-5 上，`docs/blueprint/packages.md`「装卸」）：真核心走一遍。装进管理员家目录那一层、同名目录一起拷；
//! 写错的、和出厂的撞了的、和别的包撞了的不装、什么都不留；升级换掉；卸家目录的删掉；卸出厂的记一笔、列表里标卸掉、装得回来；
//! 必需的、没装的不能卸。装卸以后 `package.list`、预设的功能当场照新的。

use std::path::PathBuf;

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::*;

/// 一份带两个功能的扩展清单，编号 `xtool`，子命令名 `command`。
fn manifest(name: &str, command: &str) -> String {
    manifest_running(name, command, "miyu-xtool")
}

/// 同 [`manifest`]，程序是 `program`：要它的功能、配置项算数的，程序得在测试程序旁边（施工 F-6 上：程序不在的当没装）。
fn manifest_running(name: &str, command: &str, program: &str) -> String {
    format!(
        "[package]\nkind = \"process\"\nprotocol = [1, 1]\nname = {{ zh = \"{name}\" }}\n\n[command]\nname = \"{command}\"\nprogram = \"{program}\"\nabout = {{ en = \"X\" }}\n\n[features.xread]\nname = {{ zh = \"读\" }}\n\n[features.xwrite]\nname = {{ zh = \"写\" }}\n"
    )
}

/// 在工作目录里放一份要装的清单（数据根外面），交回它的路径。
fn source(home: &Home, file: &str, text: &str) -> PathBuf {
    let path = home.work.join(file);
    std::fs::write(&path, text).expect("写得进");
    path
}

/// 管理员家目录那一层里的这个文件。
fn mine(home: &Home, relative: &str) -> PathBuf {
    home.root.path().join("home/alice/packages").join(relative)
}

async fn connected(home: &Home) -> Client {
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    client
}

async fn list(client: &mut Client) -> Vec<Value> {
    let reply = client.call("l", "package.list", json!({})).await;
    reply["result"]["packages"].as_array().expect("有").clone()
}

fn find<'a>(packages: &'a [Value], id: &str) -> Option<&'a Value> {
    packages.iter().find(|one| one["package"] == id)
}

/// 预设里功能 `id` 装没装（施工 F-3 下的 `preset.get`）。
async fn feature_installed(client: &mut Client, id: &str) -> Option<bool> {
    let reply = client
        .call("g", "preset.get", json!({"preset": "full"}))
        .await;
    reply["result"]["features"]
        .as_array()?
        .iter()
        .find(|one| one["id"] == id)?["installed"]
        .as_bool()
}

#[tokio::test]
async fn a_manifest_installs_with_its_files_and_counts_at_once() {
    let home = Home::new();
    let program = crate::support::extensions::Program::new();
    let path = source(
        &home,
        "xtool.toml",
        &manifest_running("工具甲", "xtool", &program.name()),
    );
    std::fs::create_dir_all(home.work.join("xtool/bin")).expect("建得了");
    std::fs::write(home.work.join("xtool/bin/data.txt"), "hi").expect("写得进");
    let mut client = connected(&home).await;
    assert_eq!(
        feature_installed(&mut client, "xread").await,
        None,
        "还没装"
    );
    let reply = client
        .call("i1", "package.install", json!({"path": path}))
        .await;
    assert_eq!(reply["result"]["package"], "xtool", "{reply}");
    assert_eq!(reply["result"]["layer"], "home");
    assert_eq!(reply["result"]["name"], "工具甲");
    assert!(mine(&home, "xtool.toml").is_file());
    assert_eq!(
        std::fs::read_to_string(mine(&home, "xtool/bin/data.txt")).expect("拷了"),
        "hi"
    );
    let packages = list(&mut client).await;
    assert_eq!(find(&packages, "xtool").expect("列了")["layer"], "home");
    assert_eq!(
        feature_installed(&mut client, "xread").await,
        Some(true),
        "带的功能当场进预设"
    );
    // 升级：同一个编号再装一次，换成新的；这一次没有同名目录，原来的文件跟着没了。
    std::fs::remove_dir_all(home.work.join("xtool")).expect("删得掉");
    let path = source(
        &home,
        "xtool.toml",
        &manifest_running("工具乙", "xtool", &program.name()),
    );
    let reply = client
        .call("i2", "package.install", json!({"path": path}))
        .await;
    assert_eq!(reply["result"]["name"], "工具乙", "{reply}");
    assert!(!mine(&home, "xtool").exists());
    assert!(
        leftovers(&home).is_empty(),
        "升级成了删掉备份：{:?}",
        leftovers(&home)
    );
    // 卸：家目录那一层的删掉。
    let reply = client
        .call("r1", "package.remove", json!({"package": "xtool"}))
        .await;
    assert_eq!(
        reply["result"],
        json!({"package": "xtool", "removed": true})
    );
    assert!(!mine(&home, "xtool.toml").exists());
    assert!(find(&list(&mut client).await, "xtool").is_none());
    assert_eq!(feature_installed(&mut client, "xread").await, None);
}

#[tokio::test]
async fn a_wrong_or_clashing_manifest_installs_nothing() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let broken = source(&home, "xbad.toml", "[package]\nkind = \"daemon\"\n");
    let reply = client
        .call("i1", "package.install", json!({"path": broken}))
        .await;
    assert_eq!(reason(&reply), Some("package_invalid"), "{reply}");
    assert_eq!(reply["error"]["data"]["line"], 2);
    assert!(
        reply["error"]["data"]["problem"]
            .as_str()
            .is_some_and(|said| said.contains("daemon")),
        "{reply}"
    );
    let shipped = source(&home, "web.toml", &manifest("网页", "xweb"));
    let reply = client
        .call("i2", "package.install", json!({"path": shipped}))
        .await;
    assert_eq!(reason(&reply), Some("package_exists"), "{reply}");
    // 读得成，可子命令名和出厂的接入QQ 撞了：拷进去以后认出来，撤回。
    let clash = source(&home, "xclash.toml", &manifest("撞了", "onebot"));
    let reply = client
        .call("i3", "package.install", json!({"path": clash}))
        .await;
    assert_eq!(reason(&reply), Some("package_invalid"), "{reply}");
    for left in ["xbad.toml", "web.toml", "xclash.toml"] {
        assert!(!mine(&home, left).exists(), "{left} 不留");
    }
    assert!(find(&list(&mut client).await, "xclash").is_none());
    for params in [
        json!({}),
        json!({"path": "xtool.toml"}),
        json!({"path": "/nowhere/x.txt"}),
        json!({"path": "/a.toml", "package": "a"}),
    ] {
        let reply = client.call("i4", "package.install", params.clone()).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{params}");
    }
}

#[tokio::test]
async fn a_shipped_package_is_removed_by_a_note_and_comes_back() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let reply = client
        .call("r1", "package.remove", json!({"package": "roleplay"}))
        .await;
    assert_eq!(reply["result"]["removed"], true, "{reply}");
    assert!(mine(&home, "roleplay.removed").is_file(), "家目录记一笔");
    let packages = list(&mut client).await;
    let roleplay = find(&packages, "roleplay").expect("照样列");
    assert_eq!(roleplay["removed"], true);
    assert_eq!(roleplay["layer"], "shipped");
    assert_eq!(
        feature_installed(&mut client, "roleplay").await,
        None,
        "卸掉的不算装了"
    );
    let reply = client
        .call("i1", "package.install", json!({"package": "roleplay"}))
        .await;
    assert_eq!(reply["result"]["package"], "roleplay", "{reply}");
    assert!(!mine(&home, "roleplay.removed").exists());
    assert!(
        find(&list(&mut client).await, "roleplay")
            .expect("回来了")
            .get("removed")
            .is_none()
    );
    assert_eq!(feature_installed(&mut client, "roleplay").await, Some(true));
    let reply = client
        .call("i2", "package.install", json!({"package": "roleplay"}))
        .await;
    assert_eq!(reason(&reply), Some("unknown_package"), "没卸的装不回来");
}

#[tokio::test]
async fn required_and_unknown_packages_cannot_be_removed() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let reply = client
        .call("r1", "package.remove", json!({"package": "basesystem"}))
        .await;
    assert_eq!(reason(&reply), Some("package_required"), "{reply}");
    assert!(!mine(&home, "basesystem.removed").exists());
    let reply = client
        .call("r2", "package.remove", json!({"package": "nothing"}))
        .await;
    assert_eq!(reason(&reply), Some("unknown_package"), "{reply}");
    let reply = client
        .call("r3", "package.remove", json!({"package": "Not An Id"}))
        .await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
}

/// 升级撞了：撤回新的，原来那一份照旧（施工 F-5 上）。
#[tokio::test]
async fn a_failed_upgrade_keeps_the_old_one() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let path = source(&home, "xkeep.toml", &manifest("旧的", "xkeep"));
    std::fs::create_dir_all(home.work.join("xkeep")).expect("建得了");
    std::fs::write(home.work.join("xkeep/v.txt"), "1").expect("写得进");
    let reply = client
        .call("i1", "package.install", json!({"path": path}))
        .await;
    assert_eq!(reply["result"]["name"], "旧的", "{reply}");
    std::fs::write(home.work.join("xkeep/v.txt"), "2").expect("写得进");
    let path = source(&home, "xkeep.toml", &manifest("新的", "onebot"));
    let reply = client
        .call("i2", "package.install", json!({"path": path}))
        .await;
    assert_eq!(reason(&reply), Some("package_invalid"), "{reply}");
    let packages = list(&mut client).await;
    assert_eq!(find(&packages, "xkeep").expect("还在")["name"], "旧的");
    assert_eq!(
        std::fs::read_to_string(mine(&home, "xkeep/v.txt")).expect("还在"),
        "1",
        "原来的文件放回去"
    );
    assert!(
        leftovers(&home).is_empty(),
        "不留暂存、备份：{:?}",
        leftovers(&home)
    );
}

/// 家目录那一层里点开头的：暂存、备份。
fn leftovers(home: &Home) -> Vec<String> {
    std::fs::read_dir(home.root.path().join("home/alice/packages"))
        .expect("读得了")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with('.'))
        .collect()
}
