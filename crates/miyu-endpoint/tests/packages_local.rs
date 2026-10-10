//! 本地库（施工 F-8 中上，设计 `31-软件包.md` 第四节）：真核心走一遍。家目录里装成的记下每个文件的路径、SHA-256、字节数，
//! `package.info`、`package.files` 照它答；升级换掉那一份，卸掉删掉；出厂的不进本地库，照现在的文件现算；以前装的、没记的
//! 核心读清单时补上（`source` 是空的）。

use std::path::PathBuf;

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::*;

/// 一份界面包的清单（不拉起程序）。
fn manifest(version: &str) -> String {
    format!(
        "[package]\nversion = \"{version}\"\nprotocol = [1, 1]\nname = {{ en = \"Tool\" }}\n\n[command]\nname = \"xtool\"\nprogram = \"miyu-xtool\"\nabout = {{ en = \"X\" }}\n\n[ui]\n"
    )
}

/// 工作目录里的包 `xtool`：清单和 `extra` 里的文件，交回文件夹。
fn source(home: &Home, version: &str, extra: &[(&str, &str)]) -> PathBuf {
    let folder = home.work.join("xtool");
    if folder.exists() {
        std::fs::remove_dir_all(&folder).expect("删得掉");
    }
    std::fs::create_dir_all(&folder).expect("建得了");
    std::fs::write(folder.join("package.toml"), manifest(version)).expect("写得进");
    for (path, text) in extra {
        let path = folder.join(path);
        std::fs::create_dir_all(path.parent().expect("有上级")).expect("建得了");
        std::fs::write(path, text).expect("写得进");
    }
    folder
}

/// 本地库里包 `id` 的那一份。
fn recorded(home: &Home, id: &str) -> PathBuf {
    home.root.path().join("state/packages/.local").join(id)
}

async fn call(client: &mut Client, method: &str, params: Value) -> Value {
    client.call("c", method, params).await
}

#[tokio::test]
async fn an_installed_package_is_recorded_listed_and_forgotten() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let folder = source(&home, "1.0", &[("bin/data.txt", "hi")]);
    let reply = call(&mut client, "package.install", json!({"path": folder})).await;
    assert_eq!(reply["result"]["package"], "xtool", "{reply}");
    assert!(recorded(&home, "xtool").join("desc").is_file(), "记下了");
    let info = call(&mut client, "package.info", json!({"package": "xtool"})).await;
    let info = &info["result"];
    assert_eq!(info["layer"], "home", "{info}");
    assert_eq!(info["version"], "1.0");
    assert_eq!(info["source"], folder.display().to_string());
    assert_eq!(info["files"], 2);
    assert_eq!(
        info["size"],
        u64::try_from(manifest("1.0").len()).expect("小") + 2
    );
    assert!(
        info["installed"]
            .as_str()
            .is_some_and(|at| at.ends_with('Z'))
    );
    let files = call(&mut client, "package.files", json!({"package": "xtool"})).await;
    assert_eq!(
        files["result"]["files"][0],
        json!({"path": "bin/data.txt", "size": 2,
               "sha256": "8f434346648f6b96df89dda901c5176b10a6d83961dd3c1ac88b59b2dc327aa4"}),
        "{files}"
    );
    assert_eq!(files["result"]["files"][1]["path"], "package.toml");
    // 升级：换成新的那一份。
    let folder = source(&home, "2.0", &[]);
    call(&mut client, "package.install", json!({"path": folder})).await;
    let info = call(&mut client, "package.info", json!({"package": "xtool"})).await;
    assert_eq!(
        (
            info["result"]["version"].clone(),
            info["result"]["files"].clone()
        ),
        (json!("2.0"), json!(1)),
        "{info}"
    );
    // 卸：本地库里那一份也删掉。
    call(&mut client, "package.remove", json!({"package": "xtool"})).await;
    assert!(!recorded(&home, "xtool").exists());
    let gone = call(&mut client, "package.info", json!({"package": "xtool"})).await;
    assert_eq!(reason(&gone), Some("unknown_package"), "{gone}");
}

#[tokio::test]
async fn a_shipped_package_is_counted_from_its_files() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let info = call(&mut client, "package.info", json!({"package": "web"})).await;
    assert_eq!(info["result"]["layer"], "shipped", "{info}");
    assert!(
        info["result"].get("installed").is_none(),
        "出厂的不进本地库"
    );
    assert!(!recorded(&home, "web").exists());
    let files = call(&mut client, "package.files", json!({"package": "web"})).await;
    assert!(
        files["result"]["files"]
            .as_array()
            .is_some_and(|all| all.iter().any(|one| one["path"] == "package.toml")),
        "{files}"
    );
    let bad = call(
        &mut client,
        "package.info",
        json!({"package": "web", "x": 1}),
    )
    .await;
    assert_eq!(reason(&bad), Some("bad_params"));
}

#[tokio::test]
async fn an_older_install_is_recorded_when_the_core_reads_its_packages() {
    let home = Home::new();
    home.write("home/alice/packages/xtool/package.toml", &manifest("0.9"));
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let info = call(&mut client, "package.info", json!({"package": "xtool"})).await;
    assert_eq!(info["result"]["version"], "0.9", "{info}");
    assert!(info["result"]["installed"].is_string(), "补上了：{info}");
    assert!(info["result"].get("source").is_none(), "补的不知道从哪装的");
    assert!(recorded(&home, "xtool").join("files").is_file());
}
