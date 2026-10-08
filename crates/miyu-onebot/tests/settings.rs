//! 桥起来时读的两项配置（施工 O-8，`onebot.md` 第一条「怎么走」第 1 条）：令牌没设、取不到的起不来；设了的照密钥文件、
//! 环境变量取；端口不写是 8301。语言照 `ui.language`，`auto` 的照系统的语言。

use miyu_endpoint::config::Environment;
use miyu_onebot::settings::{Unready, load};
use miyu_session::testkit::Script;

use crate::support::Home;

/// 改系统配置、密钥文件：测试的核心已经起来了，桥自己读，读的是磁盘上的。
fn write(home: &Home, relative: &str, text: &str) {
    std::fs::write(home.root.path().join(relative), text).expect("写得进");
}

#[tokio::test]
async fn without_a_token_the_bridge_does_not_start() {
    let home = Home::new(&Script::new([]));
    let loaded = load(&home.root, None, Some("zh_CN.UTF-8"), Environment::of(&[]));
    assert_eq!(loaded.language, "zh");
    assert_eq!(loaded.settings.err(), Some(Unready::NoToken));
    write(
        &home,
        "system/config.toml",
        "[onebot]\ntoken = { secret = \"onebot\" }\n",
    );
    let loaded = load(&home.root, None, Some("en_US"), Environment::of(&[]));
    assert_eq!(loaded.language, "en");
    assert_eq!(
        loaded.settings.err(),
        Some(Unready::NoToken),
        "引用的密钥没存"
    );
}

#[tokio::test]
async fn the_token_comes_from_the_secrets_file_or_the_environment() {
    let home = Home::new(&Script::new([]));
    write(
        &home,
        "system/config.toml",
        "[ui]\nlanguage = \"ja\"\n[onebot]\ntoken = { secret = \"onebot\" }\n",
    );
    write(&home, "system/secrets.toml", "onebot = \"from-file\"\n");
    let loaded = load(&home.root, None, Some("zh_CN"), Environment::of(&[]));
    assert_eq!(loaded.language, "ja");
    let settings = loaded.settings.expect("起得来");
    assert_eq!(settings.port, 8301);
    assert_eq!(settings.token.expose(), "from-file");
    write(
        &home,
        "system/config.toml",
        "[onebot]\nlisten = 9000\ntoken = { env = \"NAPCAT_TOKEN\" }\n",
    );
    let loaded = load(
        &home.root,
        None,
        None,
        Environment::of(&[("NAPCAT_TOKEN", "from-env")]),
    );
    let settings = loaded.settings.expect("起得来");
    assert_eq!((settings.port, settings.token.expose()), (9000, "from-env"));
}
