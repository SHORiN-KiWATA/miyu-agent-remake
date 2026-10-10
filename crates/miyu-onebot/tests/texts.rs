//! 说给人听的字（施工 O-8，`onebot.md` 第一条「给人看的字」）：字在 `resources/software/onebot/human/`，三种语言里桥说的
//! 每一句都换得出来（换不出来的会印出说法的编号），字段换进去；中文照图纸；日文照英文；换语言照新的说；握手以前照系统的
//! 语言（施工 O-20：不读配置）。O-18 多了 `start`、`stop`、`restart`、`status`、`logs` 说的；O-21 多了 `venue show` 说的、
//! 出厂的数据有问题、问题说成话；O-23 多了限流满了发进群里的那一句；O-28 下去掉桥自己的网页说的几句，`status` 多了设置在哪那一句。

use miyu_chat::{Entry, Origin, Problem, Source};
use miyu_config::Value;
use miyu_config::problem::{At, Code};
use miyu_onebot::control::{Halt, Report};
use miyu_onebot::logs::Heading;
use miyu_onebot::serve::{Failure, Notice};
use miyu_onebot::texts::{Texts, system_language};
use miyu_onebot::venue::Shown;
use miyu_store::resources::ResourceRoot;

/// 源码树里的资源目录。
fn resources() -> ResourceRoot {
    ResourceRoot::at(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"))
}

/// 一条问题：系统的 `80-x.toml` 第 1 条规则第 3 行的 `rate`，原文 `"abc"`，原因码是 `code`。
fn problem(code: Code) -> Problem {
    Problem {
        code,
        source: Source::System,
        file: "80-x.toml".to_string(),
        rule: Some(1),
        key: Some("rate".to_string()),
        at: Some(At { line: 3, column: 8 }),
        got: Some("\"abc\"".to_string()),
        suggest: None,
        why: None,
    }
}

/// 每一种问题：每一种原因码，在哪的三种（有第几条规则、只有第几行、整份文件），不认识的键有没有最近的名字，缺了的。
fn problems() -> Vec<Problem> {
    let mut all: Vec<Problem> = [
        Code::Unreadable,
        Code::TooBig,
        Code::NotUtf8,
        Code::Syntax,
        Code::UnknownKey,
        Code::WrongType,
        Code::NotAnOption,
        Code::OutOfRange,
        Code::BadFormat,
        Code::UnknownSecret,
    ]
    .into_iter()
    .map(problem)
    .collect();
    all.push(Problem {
        suggest: Some("rate".to_string()),
        key: Some("rat".to_string()),
        ..problem(Code::UnknownKey)
    });
    all.push(Problem {
        source: Source::Factory,
        file: "defaults.toml".to_string(),
        rule: None,
        ..problem(Code::OutOfRange)
    });
    all.push(Problem {
        source: Source::Factory,
        file: "defaults.toml".to_string(),
        rule: None,
        at: None,
        got: None,
        key: Some("chatty.base".to_string()),
        ..problem(Code::WrongType)
    });
    all.push(Problem {
        rule: None,
        key: None,
        at: None,
        got: None,
        why: Some("permission denied".to_string()),
        ..problem(Code::Unreadable)
    });
    all
}

/// 桥说的每一句：用法、日志、起来连上断开、起不来停了、开关和状态、`logs` 的标题、`venue show` 的
/// 每一句和每一种问题。
fn everything(texts: &Texts) -> Vec<String> {
    let mut said = vec![
        texts.usage(),
        texts.no_log("disk full"),
        texts.rate_limited(),
    ];
    for notice in [
        Notice::Listening { port: 8301 },
        Notice::Connected { bot: Some(30003) },
        Notice::Connected { bot: None },
        Notice::Disconnected { bot: Some(30003) },
        Notice::Disconnected { bot: None },
        Notice::NoToken,
    ] {
        said.push(texts.notice(&notice));
    }
    for failure in [
        Failure::Core("refused".to_string()),
        Failure::NotSpawned,
        Failure::PortInUse(8301),
        Failure::Crashed("boom".to_string()),
        Failure::Start("nope".to_string()),
        Failure::Factory(problems()),
    ] {
        said.push(texts.failure(&failure));
    }
    for shown in [
        Shown::None,
        Shown::Defaults,
        Shown::Problems,
        Shown::BadVenue("qq:room:1".to_string()),
    ] {
        said.push(texts.shown(&shown));
    }
    said.push(texts.entry(
        "rate",
        &Entry {
            value: Value::Text("5/300s".into()),
            origin: Origin {
                source: Source::Factory,
                file: "50-defaults.toml".to_string(),
                rule: 1,
                line: 15,
            },
        },
    ));
    for problem in problems() {
        said.push(texts.problem(&problem));
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
        Report::Listen(8301),
        Report::Page,
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
    let listening = Notice::Listening { port: 8301 };
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
        "还没设令牌（onebot.token），NapCat 连进来会被拒。到网页的软件后台生成一个（miyu web --package onebot），填进 NapCat，几秒内就连上。"
    );
    assert_eq!(
        texts.usage(),
        "用法：miyu onebot start | stop | restart | status | logs [-f] | venue show <场所>（serve 只由核心拉起）"
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
    assert_eq!(texts.rate_limited(), "这会儿叫的人太多了，过几分钟再来吧。");
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
        texts.report(&Report::Listen(8301)),
        "NapCat 连 ws://127.0.0.1:8301/ws。"
    );
    assert_eq!(
        texts.report(&Report::Page),
        "设置和状态在网页的软件后台：miyu web --package onebot"
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
        "usage: miyu onebot start | stop | restart | status | logs [-f] | venue show <venue> (serve is started by the core only)"
    );
    assert_eq!(
        texts.report(&Report::Halted(Halt::ConfigError)),
        "The QQ bridge stopped and the core will not start it again: a configuration error or a port in use (exit code 1). Fix it, then run miyu onebot restart."
    );
    texts.speak("zh").expect("读得出来");
    assert_eq!(texts.report(&Report::NoNapcat), "NapCat 还没连上。");
}

#[test]
fn before_the_handshake_the_system_language_is_spoken() {
    assert_eq!(system_language(Some("zh_CN.UTF-8")), "zh");
    assert_eq!(system_language(Some("ja_JP")), "ja");
    assert_eq!(system_language(Some("de_DE")), "en");
    assert_eq!(system_language(None), "en");
}

#[test]
fn venue_show_and_problems_read_as_drawn() {
    let texts = Texts::load(resources(), "zh").expect("读得出来");
    let said: Vec<String> = problems().iter().map(|one| texts.problem(one)).collect();
    assert_eq!(
        said,
        [
            "系统 80-x.toml 第 1 条规则，第 3 行：读不了：",
            "系统 80-x.toml 第 1 条规则，第 3 行：超过 1 MiB，不读",
            "系统 80-x.toml 第 1 条规则，第 3 行：不是 UTF-8，不读",
            "系统 80-x.toml 第 1 条规则，第 3 行：TOML 写法不对：",
            "系统 80-x.toml 第 1 条规则，第 3 行：不认识 rate",
            "系统 80-x.toml 第 1 条规则，第 3 行：rate 的类型不对：\"abc\"",
            "系统 80-x.toml 第 1 条规则，第 3 行：rate 不是能选的值：\"abc\"",
            "系统 80-x.toml 第 1 条规则，第 3 行：rate 超出范围：\"abc\"",
            "系统 80-x.toml 第 1 条规则，第 3 行：rate 写法不对：\"abc\"",
            "系统 80-x.toml 第 1 条规则，第 3 行：rate：unknown_secret",
            "系统 80-x.toml 第 1 条规则，第 3 行：不认识 rat，是不是想写 rate？",
            "出厂 defaults.toml 第 3 行：rate 超出范围：\"abc\"",
            "出厂 defaults.toml：缺了 chatty.base",
            "系统 80-x.toml：读不了：permission denied",
        ]
    );
    assert_eq!(
        texts.failure(&Failure::Factory(problems()[..2].to_vec())),
        "QQ 桥的出厂数据有问题（是打包的错），起不来：\n  \
         系统 80-x.toml 第 1 条规则，第 3 行：读不了：\n  \
         系统 80-x.toml 第 1 条规则，第 3 行：超过 1 MiB，不读"
    );
    assert_eq!(texts.shown(&Shown::None), "没有规则设到这个场所。");
    assert_eq!(
        texts.shown(&Shown::Defaults),
        "没列出的参数照出厂的 defaults.toml。"
    );
    assert_eq!(
        texts.shown(&Shown::Problems),
        "读文件时发现的问题（写错的那一项、那一条规则、那一份文件不用，别的照用）："
    );
    assert_eq!(
        texts.shown(&Shown::BadVenue("qq:room:1".to_string())),
        "认不出场所编号 qq:room:1。写成 <平台>:group:<群号> 或 <平台>:private:<号>，例如 qq:group:123456。"
    );
    let english = Texts::load(resources(), "en").expect("读得出来");
    assert_eq!(
        english.problem(&problems()[12]),
        "factory defaults.toml: chatty.base is missing"
    );
}
