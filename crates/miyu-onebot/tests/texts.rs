//! 说给人听的字（施工 O-8，`onebot.md` 第一条「给人看的字」）：字在 `resources/software/onebot/human/`，三种语言里桥说的
//! 每一句都换得出来（换不出来的会印出说法的编号），字段换进去；中文照图纸；日文照英文；换语言照新的说。

use miyu_onebot::serve::{Failure, Notice};
use miyu_onebot::settings::{Unready, system_language};
use miyu_onebot::texts::Texts;
use miyu_store::resources::ResourceRoot;

/// 源码树里的资源目录。
fn resources() -> ResourceRoot {
    ResourceRoot::at(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"))
}

/// 桥说的每一句：用法、日志、起来连上断开、起不来停了、读配置起不来。
fn everything(texts: &Texts) -> Vec<String> {
    let mut said = vec![texts.usage(), texts.no_log("disk full")];
    for notice in [
        Notice::Listening {
            port: 8301,
            language: "zh".to_string(),
        },
        Notice::Connected { bot: Some(30003) },
        Notice::Connected { bot: None },
        Notice::Disconnected { bot: Some(30003) },
        Notice::Disconnected { bot: None },
    ] {
        said.push(texts.notice(&notice));
    }
    for failure in [
        Failure::Core("refused".to_string()),
        Failure::CoreGone,
        Failure::PortInUse(8301),
        Failure::Crashed("boom".to_string()),
        Failure::Start("nope".to_string()),
    ] {
        said.push(texts.failure(&failure));
    }
    for unready in [Unready::NoToken, Unready::BadPort] {
        said.push(texts.unready(&unready));
    }
    said
}

#[test]
fn every_sentence_turns_into_words_in_every_language() {
    for language in ["zh", "en", "ja"] {
        let texts = Texts::load(resources(), language).expect("出厂的字读得出来");
        for said in everything(&texts) {
            assert!(
                !said.contains("software/onebot/"),
                "{language}：换不出来：{said}"
            );
            assert!(!said.trim().is_empty(), "{language}：空的");
        }
    }
}

#[test]
fn chinese_reads_as_drawn_and_fields_go_in() {
    let texts = Texts::load(resources(), "zh").expect("读得出来");
    let listening = Notice::Listening {
        port: 8301,
        language: "zh".to_string(),
    };
    assert_eq!(
        texts.notice(&listening),
        "在 127.0.0.1:8301 等 NapCat 连进来。"
    );
    assert_eq!(
        texts.notice(&Notice::Disconnected { bot: Some(30003) }),
        "NapCat 断开了（30003），等它重连。"
    );
    assert_eq!(
        texts.failure(&Failure::Core("refused".to_string())),
        "连不上核心：refused"
    );
    assert_eq!(
        texts.unready(&Unready::NoToken),
        "还没设 QQ 桥的令牌（onebot.token）。先存一个：miyu login onebot，再写进系统配置：miyu config set --system onebot.token '{ secret = \"onebot\" }'。NapCat 那边填同一个令牌。"
    );
    assert_eq!(texts.usage(), "用法：miyu-onebot serve");
}

#[test]
fn japanese_reads_as_english_and_the_language_can_change() {
    let english = Texts::load(resources(), "en").expect("读得出来");
    let mut texts = Texts::load(resources(), "ja").expect("读得出来");
    assert_eq!(everything(&texts), everything(&english));
    assert_eq!(texts.usage(), "usage: miyu-onebot serve");
    texts.speak("zh").expect("读得出来");
    assert_eq!(texts.failure(&Failure::CoreGone), "核心不在了，QQ 桥停下。");
}

#[test]
fn before_the_config_is_read_the_system_language_is_spoken() {
    assert_eq!(system_language(Some("zh_CN.UTF-8")), "zh");
    assert_eq!(system_language(Some("ja_JP")), "ja");
    assert_eq!(system_language(Some("de_DE")), "en");
    assert_eq!(system_language(None), "en");
}
