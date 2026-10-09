//! `miyu-onebot` 在标准错误上说给人听的字（`onebot.md` 第一条「样子」「出错」「给人看的字」）：字放在
//! `resources/software/onebot/human/{zh,en,ja}.json`，照 [`Human::load`] 读（`store/resources.md`「怎么走」第 3 条），说法的
//! 编号是 `software/onebot/<哪一句>`。这里只管挑哪一句、换进什么字段。`start`、`stop`、`restart`、`status`、`logs` 说的
//! （施工 O-18）、`venue show` 说的和场所规则的问题说成话（施工 O-21）、限流满了发进群里的那一句（施工 O-23）也在这里。
//!
//! 说话的语言：握手以前照系统的语言（[`system_language`]；施工 O-20 起桥不读配置，不看 `ui.language`），握手以后照核心回的
//! `language`。日文没有专门写的，`ja.json` 照英文写，和核心拒绝时的话一样（`protocol.md`「握手」`language`）。

use miyu_chat::{Entry, Problem, Source};
use miyu_config::problem::Code;
use miyu_kernel::event::Said;
use miyu_store::human::{Human, HumanError};
use miyu_store::resources::ResourceRoot;

use crate::control::{Halt, Report};
use crate::logs::Heading;
use crate::open::Opening;
use crate::serve::{Failure, Notice};
use crate::venue::Shown;

/// 这个包的说法编号的前缀：软件包的说法照它在资源目录里的位置起（`store/resources.md`「怎么走」第 3 条第 4 款）。
const PREFIX: &str = "software/onebot/";

/// 握手以前说话的语言：照系统的语言 `locale`（`miyu_store::env::locale`），`zh`、`ja` 开头的照它，别的、没有的说英文。和核心
/// 照握手的 `locale` 算 `ui.language = auto` 的是同一个规矩（「施工时定的」第 44 条）。
pub fn system_language(locale: Option<&str>) -> &'static str {
    match locale {
        Some(locale) if locale.starts_with("zh") => "zh",
        Some(locale) if locale.starts_with("ja") => "ja",
        _ => "en",
    }
}

/// 读好的一种语言的字。
#[derive(Debug, Clone)]
pub struct Texts {
    /// 资源目录：换语言时再读。
    resources: ResourceRoot,
    /// 现在说的语言：`zh`、`en`、`ja` 这样的写法。
    language: String,
    /// 这种语言的字。
    human: Human,
}

impl Texts {
    /// 照资源目录 `resources` 读 `language` 这一种。
    ///
    /// # Errors
    ///
    /// 有一份给人看的字读得到却读不懂（[`Human::load`]）。
    pub fn load(resources: ResourceRoot, language: &str) -> Result<Texts, HumanError> {
        let human = Human::load(&resources, language)?;
        Ok(Texts {
            resources,
            language: language.to_string(),
            human,
        })
    }

    /// 换成说 `language`；已经是它的不再读。
    ///
    /// # Errors
    ///
    /// 那种语言的字读不懂：还照原来的说。
    pub fn speak(&mut self, language: &str) -> Result<(), HumanError> {
        if language != self.language {
            *self = Texts::load(self.resources.clone(), language)?;
        }
        Ok(())
    }

    /// 资源目录。
    pub fn resources(&self) -> &ResourceRoot {
        &self.resources
    }

    /// 用法不对。
    pub fn usage(&self) -> String {
        self.say("usage", &[])
    }

    /// 运行日志装不上（照样跑）。
    pub fn no_log(&self, reason: &str) -> String {
        self.say("no-log", &[("reason", reason.to_string())])
    }

    /// 限流满了、别人冲她来时发进群里的那一句（施工 O-23，`onebot.md` 第一条「群里怎么叫她」第 7 条）：说话的是桥，不是她。
    pub fn rate_limited(&self) -> String {
        self.say("group/rate-limited", &[])
    }

    /// 起来了（令牌没设的说怎么设）、连上了、断开了。号没认出来的不说号。
    pub fn notice(&self, notice: &Notice) -> String {
        match notice {
            Notice::Listening { port, .. } => {
                self.say("notice/listening", &[("port", port.to_string())])
            }
            Notice::Connected { bot: Some(bot) } => {
                self.say("notice/connected-as", &[("bot", bot.to_string())])
            }
            Notice::Connected { bot: None } => self.say("notice/connected", &[]),
            Notice::Disconnected { bot: Some(bot) } => {
                self.say("notice/disconnected-as", &[("bot", bot.to_string())])
            }
            Notice::Disconnected { bot: None } => self.say("notice/disconnected", &[]),
            Notice::Web { port } => self.say("notice/web", &[("port", port.to_string())]),
            Notice::NoToken => self.say("notice/no-token", &[]),
        }
    }

    /// 起不来、停了。
    pub fn failure(&self, failure: &Failure) -> String {
        match failure {
            Failure::Core(reason) => self.say("failure/core", &[("reason", reason.clone())]),
            Failure::NotSpawned => self.say("failure/not-spawned", &[]),
            Failure::PortInUse(port) => {
                self.say("failure/port-in-use", &[("port", port.to_string())])
            }
            Failure::WebPortInUse(port) => {
                self.say("failure/web-port-in-use", &[("port", port.to_string())])
            }
            Failure::Crashed(reason) => self.say("failure/crashed", &[("reason", reason.clone())]),
            Failure::Start(reason) => self.say("failure/start", &[("reason", reason.clone())]),
            Failure::Factory(problems) => {
                let mut said = self.say("failure/factory", &[]);
                for problem in problems {
                    said.push_str(&format!("\n  {}", self.problem(problem)));
                }
                said
            }
        }
    }

    /// `venue show` 说的几句（施工 O-21）。
    pub fn shown(&self, shown: &Shown) -> String {
        match shown {
            Shown::None => self.say("venue/none", &[]),
            Shown::Defaults => self.say("venue/defaults", &[]),
            Shown::Problems => self.say("venue/problems", &[]),
            Shown::BadVenue(venue) => self.say("venue/bad-venue", &[("venue", venue.clone())]),
        }
    }

    /// 规则设到的一项（施工 O-21）：键 `key`、值照 TOML 写、来处。
    pub fn entry(&self, key: &str, entry: &Entry) -> String {
        let origin = &entry.origin;
        let from = self.place(
            origin.source,
            &origin.file,
            Some(origin.rule),
            Some(origin.line),
        );
        self.say(
            "venue/entry",
            &[
                ("key", key.to_string()),
                ("value", entry.value.toml()),
                ("from", from),
            ],
        )
    }

    /// 一条场所规则的问题说成话（施工 O-21，「施工时定的」第 56 条）：在哪，加错在哪（一种原因码一句；出厂参数缺了的是
    /// `wrong_type` 没有原文，另说一句）。
    pub fn problem(&self, problem: &Problem) -> String {
        let at = self.place(
            problem.source,
            &problem.file,
            problem.rule,
            problem.at.map(|at| at.line),
        );
        let key = ("key", problem.key.clone().unwrap_or_default());
        let got = ("got", problem.got.clone().unwrap_or_default());
        let why = ("why", problem.why.clone().unwrap_or_default());
        let what = match (problem.code, &problem.suggest) {
            (Code::Unreadable, _) => self.say("problem/unreadable", &[why]),
            (Code::TooBig, _) => self.say("problem/too-big", &[]),
            (Code::NotUtf8, _) => self.say("problem/not-utf8", &[]),
            (Code::Syntax, _) => self.say("problem/syntax", &[why]),
            (Code::UnknownKey, Some(suggest)) => {
                self.say("problem/unknown-key", &[key, ("suggest", suggest.clone())])
            }
            (Code::UnknownKey, None) => self.say("problem/unknown-key-plain", &[key]),
            (Code::WrongType, _) if problem.got.is_none() => self.say("problem/missing", &[key]),
            (Code::WrongType, _) => self.say("problem/wrong-type", &[key, got]),
            (Code::NotAnOption, _) => self.say("problem/not-an-option", &[key, got]),
            (Code::OutOfRange, _) => self.say("problem/out-of-range", &[key, got]),
            (Code::BadFormat, _) => self.say("problem/bad-format", &[key, got]),
            // 群聊内核、读文件报不出别的原因码：照原样印。
            (code, _) => self.say("problem/other", &[key, ("code", code.as_str().to_string())]),
        };
        self.say("venue/problem", &[("at", at), ("what", what)])
    }

    /// 在哪：出厂或系统、文件名，有第几条规则、第几行的带上（第几条规则只和第几行一起说）。
    fn place(
        &self,
        source: Source,
        file: &str,
        rule: Option<usize>,
        line: Option<usize>,
    ) -> String {
        let source = match source {
            Source::Factory => self.say("venue/factory", &[]),
            Source::System => self.say("venue/system", &[]),
        };
        let mut fields = vec![("source", source), ("file", file.to_string())];
        let key = match (rule, line) {
            (Some(rule), Some(line)) => {
                fields.extend([("rule", rule.to_string()), ("line", line.to_string())]);
                "venue/rule"
            }
            (None, Some(line)) => {
                fields.push(("line", line.to_string()));
                "venue/line"
            }
            (_, None) => "venue/file",
        };
        self.say(key, &fields)
    }

    /// `miyu-onebot web` 说的（施工 O-16）。
    pub fn opening(&self, opening: &Opening) -> String {
        match opening {
            Opening::NotRunning(port) => {
                self.say("open/not-running", &[("port", port.to_string())])
            }
            Opening::First => self.say("open/first", &[]),
            Opening::Opened(url) => self.say("open/opened", &[("url", url.clone())]),
            Opening::PrintHint => self.say("open/print-hint", &[]),
            Opening::OpenThis => self.say("open/open-this", &[]),
            Opening::CodeWarning => self.say("open/code-warning", &[]),
        }
    }

    /// `start`、`stop`、`restart`、`status` 说的一句（施工 O-18）。
    pub fn report(&self, report: &Report) -> String {
        match report {
            Report::Started => self.say("control/started", &[]),
            Report::Stopped => self.say("control/stopped", &[]),
            Report::Restarted => self.say("control/restarted", &[]),
            Report::Off => self.say("status/off", &[]),
            Report::Starting => self.say("status/starting", &[]),
            Report::Running(pid) => self.say("status/running", &[("pid", pid.to_string())]),
            Report::Waiting { seconds, failures } => self.say(
                "status/waiting",
                &[
                    ("seconds", seconds.to_string()),
                    ("failures", failures.to_string()),
                ],
            ),
            Report::Halted(halt) => self.say("status/stopped", &[("reason", self.halt(halt))]),
            Report::Stderr => self.say("status/stderr", &[]),
            Report::Other(state) => self.say("status/other", &[("state", state.clone())]),
            Report::Missing => self.say("status/missing", &[]),
            Report::Napcat {
                implementation,
                version,
                bot,
            } => self.say(
                "status/napcat",
                &[
                    ("implementation", implementation.clone()),
                    ("version", version.clone()),
                    ("bot", bot.clone()),
                ],
            ),
            Report::NapcatBot(bot) => self.say("status/napcat-bot", &[("bot", bot.clone())]),
            Report::NoNapcat => self.say("status/no-napcat", &[]),
            Report::Ports { listen, web } => self.say(
                "status/ports",
                &[("listen", listen.to_string()), ("web", web.to_string())],
            ),
        }
    }

    /// 停下的原因；不认识的照原样。
    fn halt(&self, halt: &Halt) -> String {
        match halt {
            Halt::ConfigError => self.say("reason/config_error", &[]),
            Halt::FailedRepeatedly(failures) => self.say(
                "reason/failed_repeatedly",
                &[("failures", failures.to_string())],
            ),
            Halt::NotInstalled => self.say("reason/not_installed", &[]),
            Halt::CannotStart => self.say("reason/cannot_start", &[]),
            Halt::ProtocolMismatch => self.say("reason/protocol_mismatch", &[]),
            Halt::Other(code) => code.clone(),
        }
    }

    /// `logs` 的标题、还没有运行日志那一句（施工 O-18）。
    pub fn heading(&self, heading: &Heading) -> String {
        match heading {
            Heading::Stderr(path) => self.say("logs/stderr", &[("path", path.clone())]),
            Heading::Log(path) => self.say("logs/log", &[("path", path.clone())]),
            Heading::None(path) => self.say("logs/none", &[("path", path.clone())]),
        }
    }

    /// 说 `key` 那一句，换进 `fields`。换不出来的（没有这一句、少了字段：是 bug，`tests/texts.rs` 守着）不吞掉：
    /// 照实印出说法的编号和字段，再记一行运行日志。
    fn say(&self, key: &str, fields: &[(&str, String)]) -> String {
        let said = fields.iter().fold(
            Said::new(format!("{PREFIX}{key}")),
            |said, (field, value)| said.with(field, value.clone()),
        );
        self.human.say(&said).unwrap_or_else(|| {
            tracing::warn!(target: crate::TARGET, key = %said.key, "human text missing");
            format!("{} {:?}", said.key, said.fields)
        })
    }
}
