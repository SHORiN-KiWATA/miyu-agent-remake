//! 说给人听的字（施工 O-8，`onebot.md` 第一条「给人看的字」）：字在 `resources/software/onebot/human/`，三种语言里桥说的
//! 每一句都换得出来（换不出来的会印出说法的编号），字段换进去；中文照图纸；日文照英文；换语言照新的说。O-18 多了
//! `start`、`stop`、`restart`、`status`、`logs` 说的。

use miyu_onebot::control::{Halt, Report};
use miyu_onebot::logs::Heading;
use miyu_onebot::open::Opening;
use miyu_onebot::serve::{Failure, Notice};
use miyu_onebot::settings::{Unready, system_language};
use miyu_onebot::texts::Texts;
use miyu_store::resources::ResourceRoot;

/// 源码树里的资源目录。
fn resources() -> ResourceRoot {
    ResourceRoot::at(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"))
}

/// 桥说的每一句：用法、日志、起来连上断开、起不来停了、读配置起不来、`miyu-onebot web`、开关和状态、`logs` 的标题。
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
        Notice::Web { port: 8302 },
        Notice::NoToken,
    ] {
        said.push(texts.notice(&notice));
    }
    for failure in [
        Failure::Core("refused".to_string()),
        Failure::NotSpawned,
        Failure::PortInUse(8301),
        Failure::WebPortInUse(8302),
        Failure::Crashed("boom".to_string()),
        Failure::Start("nope".to_string()),
    ] {
        said.push(texts.failure(&failure));
    }
    for unready in [Unready::BadPort, Unready::BadWebPort] {
        said.push(texts.unready(&unready));
    }
    for opening in [
        Opening::NotRunning(8302),
        Opening::First,
        Opening::Opened("http://127.0.0.1:8302".to_string()),
        Opening::PrintHint,
        Opening::OpenThis,
        Opening::CodeWarning,
    ] {
        said.push(texts.opening(&opening));
    }
    for halt in [
        Halt::ConfigError,
        Halt::FailedRepeatedly(5),
        Halt::NotInstalled,
        Halt::CannotStart,
        Halt::ProtocolMismatch,
        Halt::Other("new_reason".to_string()),
    ] {
        said.push(texts.report(&Report::Halted(halt)));
    }
    for report in [
        Report::Started,
        Report::Stopped,
        Report::Restarted,
        Report::Off,
        Report::Starting,
        Report::Running(4242),
        Report::Waiting {
            seconds: 2,
            failures: 1,
        },
        Report::Stderr,
        Report::Other("dreaming".to_string()),
        Report::Missing,
        Report::Napcat {
            implementation: "NapCat.Onebot".to_string(),
            version: "4.8.2".to_string(),
            bot: "30003".to_string(),
        },
        Report::NapcatBot("30003".to_string()),
        Report::NoNapcat,
        Report::Ports {
            listen: 8301,
            web: 8302,
        },
    ] {
        said.push(texts.report(&report));
    }
    for heading in [
        Heading::Stderr("state/logs/onebot.stderr".to_string()),
        Heading::Log("state/logs/onebot.log".to_string()),
        Heading::None("state/logs/onebot.log".to_string()),
    ] {
        said.push(texts.heading(&heading));
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
        texts.notice(&Notice::NoToken),
        "还没设令牌（onebot.token），NapCat 连进来会被拒。到 WebUI 的「连接」页生成一个，填进 NapCat，几秒内就连上。"
    );
    assert_eq!(
        texts.usage(),
        "用法：miyu onebot start | stop | restart | status | logs [-f] | web [--print]（serve 只由核心拉起）"
    );
    assert_eq!(
        texts.notice(&Notice::Web { port: 8302 }),
        "QQ 桥的网页在 http://127.0.0.1:8302，用 miyu-onebot web 打开。"
    );
    assert_eq!(
        texts.failure(&Failure::WebPortInUse(8302)),
        "端口 8302 被占了。换一个：miyu config set --system onebot.web <端口>。"
    );
    assert_eq!(
        texts.opening(&Opening::NotRunning(8302)),
        "127.0.0.1:8302 上没有 QQ 桥的网页。先 miyu onebot start；改过 onebot.web 的，miyu onebot restart。"
    );
    assert_eq!(
        texts.failure(&Failure::NotSpawned),
        "没等到核心的握手回应。miyu-onebot serve 只由核心拉起：用 miyu onebot start 打开 QQ 桥。"
    );
    assert_eq!(
        texts.report(&Report::Started),
        "QQ 桥开了：核心拉起它，以后核心每次起来都拉起它。"
    );
    assert_eq!(
        texts.report(&Report::Stopped),
        "QQ 桥关了：核心停下它，以后不再拉起。"
    );
    assert_eq!(texts.report(&Report::Restarted), "QQ 桥重新拉起了。");
    assert_eq!(
        texts.report(&Report::Off),
        "QQ 桥关着。用 miyu onebot start 打开。"
    );
    assert_eq!(texts.report(&Report::Starting), "QQ 桥正在起来。");
    assert_eq!(
        texts.report(&Report::Running(4242)),
        "QQ 桥在跑（进程 4242）。"
    );
    assert_eq!(
        texts.report(&Report::Waiting {
            seconds: 2,
            failures: 1
        }),
        "QQ 桥退出了，2 秒后再拉起（连续失败 1 次）。"
    );
    assert_eq!(
        texts.report(&Report::Halted(Halt::ConfigError)),
        "QQ 桥停下了，核心不再拉起它：配置错了或者端口被占（退出码 1）。改好以后 miyu onebot restart。"
    );
    assert_eq!(
        texts.report(&Report::Halted(Halt::FailedRepeatedly(5))),
        "QQ 桥停下了，核心不再拉起它：连续失败了 5 次。看 miyu onebot logs，修好以后 miyu onebot restart。"
    );
    assert_eq!(
        texts.report(&Report::Halted(Halt::NotInstalled)),
        "QQ 桥停下了，核心不再拉起它：miyu 旁边没有 miyu-onebot 这个程序。"
    );
    assert_eq!(
        texts.report(&Report::Halted(Halt::CannotStart)),
        "QQ 桥停下了，核心不再拉起它：起不来：系统不让起，或者建不了它的目录、标准错误的文件。"
    );
    assert_eq!(
        texts.report(&Report::Halted(Halt::ProtocolMismatch)),
        "QQ 桥停下了，核心不再拉起它：清单说的协议版本和核心的对不上。"
    );
    assert_eq!(
        texts.report(&Report::Halted(Halt::Other("new_reason".to_string()))),
        "QQ 桥停下了，核心不再拉起它：new_reason",
        "不认识的原因照原样"
    );
    assert_eq!(texts.report(&Report::Stderr), "它的标准错误的最后几行：");
    assert_eq!(
        texts.report(&Report::Other("dreaming".to_string())),
        "QQ 桥的状态：dreaming"
    );
    assert_eq!(
        texts.report(&Report::Missing),
        "核心那边没有 onebot 这个软件包：清单不在、或者写错了，用 miyu check 查一下。"
    );
    assert_eq!(
        texts.report(&Report::Napcat {
            implementation: "NapCat.Onebot".to_string(),
            version: "4.8.2".to_string(),
            bot: "30003".to_string(),
        }),
        "NapCat 连上了：NapCat.Onebot 4.8.2，机器人 30003。"
    );
    assert_eq!(
        texts.report(&Report::NapcatBot("30003".to_string())),
        "NapCat 连上了：机器人 30003。"
    );
    assert_eq!(texts.report(&Report::NoNapcat), "NapCat 还没连上。");
    assert_eq!(
        texts.report(&Report::Ports {
            listen: 8301,
            web: 8302
        }),
        "NapCat 连 ws://127.0.0.1:8301/ws，网页在 http://127.0.0.1:8302（miyu onebot web 打开）。"
    );
    assert_eq!(
        texts.heading(&Heading::Stderr("a/onebot.stderr".to_string())),
        "—— 标准错误 a/onebot.stderr ——"
    );
    assert_eq!(
        texts.heading(&Heading::Log("a/onebot.log".to_string())),
        "—— 运行日志 a/onebot.log ——"
    );
    assert_eq!(
        texts.heading(&Heading::None("a/onebot.log".to_string())),
        "还没有运行日志：a/onebot.log"
    );
}

#[test]
fn japanese_reads_as_english_and_the_language_can_change() {
    let english = Texts::load(resources(), "en").expect("读得出来");
    let mut texts = Texts::load(resources(), "ja").expect("读得出来");
    assert_eq!(everything(&texts), everything(&english));
    assert_eq!(
        texts.usage(),
        "usage: miyu onebot start | stop | restart | status | logs [-f] | web [--print] (serve is started by the core only)"
    );
    assert_eq!(
        texts.report(&Report::Halted(Halt::ConfigError)),
        "The QQ bridge stopped and the core will not start it again: a configuration error or a port in use (exit code 1). Fix it, then run miyu onebot restart."
    );
    texts.speak("zh").expect("读得出来");
    assert_eq!(texts.report(&Report::NoNapcat), "NapCat 还没连上。");
}

#[test]
fn before_the_config_is_read_the_system_language_is_spoken() {
    assert_eq!(system_language(Some("zh_CN.UTF-8")), "zh");
    assert_eq!(system_language(Some("ja_JP")), "ja");
    assert_eq!(system_language(Some("de_DE")), "en");
    assert_eq!(system_language(None), "en");
}
