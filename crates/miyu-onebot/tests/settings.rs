//! 桥读的几项配置（施工 O-8，`onebot.md` 第一条「怎么走」第 1 条）：令牌读成三种：没写引用、写了引用取不到（密钥没存、环境
//! 变量没设）、取到了，都照样读得出来（O-16 补、补二：桥照样起来，NapCat 连进来 401）；设了的照密钥文件、环境变量取；端口
//! 不写是 8301。语言照 `ui.language`，`auto` 的照系统的语言。

use miyu_config::secret::Secret;
use miyu_endpoint::config::Environment;
use miyu_onebot::settings::{Token, load};
use miyu_session::testkit::Script;

use crate::support::Home;

/// 改系统配置、密钥文件：测试的核心已经起来了，桥自己读，读的是磁盘上的。
fn write(home: &Home, relative: &str, text: &str) {
    std::fs::write(home.root.path().join(relative), text).expect("写得进");
}

#[tokio::test]
async fn without_a_token_the_settings_still_load() {
    let home = Home::new(&Script::new([]));
    let loaded = load(&home.root, None, Some("zh_CN.UTF-8"), Environment::of(&[]));
    assert_eq!(loaded.language, "zh");
    let settings = loaded.settings.expect("没设令牌也读得出来");
    assert_eq!(
        (settings.port, settings.web, settings.token),
        (8301, 8302, Token::Unset)
    );
    write(
        &home,
        "system/config.toml",
        "[onebot]\ntoken = { secret = \"onebot\" }\n",
    );
    let loaded = load(&home.root, None, Some("en_US"), Environment::of(&[]));
    assert_eq!(loaded.language, "en");
    assert_eq!(
        loaded.settings.expect("读得出来").token,
        Token::Missing,
        "引用的密钥没存"
    );
    write(
        &home,
        "system/config.toml",
        "[onebot]\ntoken = { env = \"NAPCAT_TOKEN\" }\n",
    );
    let loaded = load(&home.root, None, None, Environment::of(&[]));
    assert_eq!(
        loaded.settings.expect("读得出来").token,
        Token::Missing,
        "引用的环境变量没设"
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
    assert_eq!(
        (settings.port, settings.web),
        (8301, 8302),
        "出厂的两个端口"
    );
    assert_eq!(
        settings.token.secret().map(Secret::expose),
        Some("from-file")
    );
    write(
        &home,
        "system/config.toml",
        "[onebot]\nlisten = 9000\nweb = 9001\ntoken = { env = \"NAPCAT_TOKEN\" }\n",
    );
    let loaded = load(
        &home.root,
        None,
        None,
        Environment::of(&[("NAPCAT_TOKEN", "from-env")]),
    );
    let settings = loaded.settings.expect("起得来");
    assert_eq!(
        (
            settings.port,
            settings.web,
            settings.token.secret().map(Secret::expose)
        ),
        (9000, 9001, Some("from-env"))
    );
}
