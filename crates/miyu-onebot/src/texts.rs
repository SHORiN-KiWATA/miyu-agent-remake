//! `miyu-onebot` 在标准错误上说给人听的字（`onebot.md` 第一条「样子」「出错」「给人看的字」）：字放在
//! `resources/software/onebot/human/{zh,en,ja}.json`，照 [`Human::load`] 读（`store/resources.md`「怎么走」第 3 条），说法的
//! 编号是 `software/onebot/<哪一句>`。这里只管挑哪一句、换进什么字段。
//!
//! 说话的语言：读配置以前照系统的语言，读了配置照 `ui.language`（`auto` 的照系统的语言），握手以后照核心回的
//! `language`。日文没有专门写的，`ja.json` 照英文写，和核心拒绝时的话一样（`protocol.md`「握手」`language`）。

use miyu_kernel::event::Said;
use miyu_store::human::{Human, HumanError};
use miyu_store::resources::ResourceRoot;

use crate::serve::{Failure, Notice};
use crate::settings::Unready;

/// 这个包的说法编号的前缀：软件包的说法照它在资源目录里的位置起（`store/resources.md`「怎么走」第 3 条第 4 款）。
const PREFIX: &str = "software/onebot/";

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

    /// 起来了、连上了、断开了。号没认出来的不说号。
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
        }
    }

    /// 起不来、停了。
    pub fn failure(&self, failure: &Failure) -> String {
        match failure {
            Failure::Core(reason) => self.say("failure/core", &[("reason", reason.clone())]),
            Failure::CoreGone => self.say("failure/core-gone", &[]),
            Failure::PortInUse(port) => {
                self.say("failure/port-in-use", &[("port", port.to_string())])
            }
            Failure::Crashed(reason) => self.say("failure/crashed", &[("reason", reason.clone())]),
            Failure::Start(reason) => self.say("failure/start", &[("reason", reason.clone())]),
        }
    }

    /// 读配置时就起不来。
    pub fn unready(&self, unready: &Unready) -> String {
        match unready {
            Unready::NoToken => self.say("unready/no-token", &[]),
            Unready::BadPort => self.say("unready/bad-port", &[]),
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
