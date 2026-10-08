//! 桥读的配置（`onebot.md` 第一条「对外的样子」「怎么走」第 1、2 条）：系统配置里的 `onebot.listen`、`onebot.web`（施工
//! O-16，第二条）、`onebot.token`。起来时读一次；之后照 [`Reload`] 重读：NapCat 的令牌对不上时、WebUI 的 `/status`、
//! `/token`、`/apply`（O-16 补二，`crate::current` 管桥手里的那一份）。
//!
//! 三项由核心替桥声明（`miyu_core::settings::OnebotSettings`，权宜，9-1 挪进桥自己的清单）。桥照核心登记的全部清单读
//! 系统配置、个人设置、密钥文件（`miyu_endpoint::config::Config::load`，只读，同一份代码），令牌照引用取：`{ secret }` 照
//! 密钥文件，`{ env }` 照桥自己的环境。密钥从不经协议交出去（`config.md` 第九条），桥要用值，只能自己读。
//!
//! 令牌没设、引用的密钥或环境变量取不到，照样读得出来（[`Token::Unset`]、[`Token::Missing`]）：桥照样起来，NapCat 连进来
//! 一律 401，设了不用重启（第 1、2 条，施工 O-16 补、补二；`18-通讯平台.md` 第三节「还没配好就 `start`」）。读不出来的只有
//! 两个端口。
//!
//! 读配置在连核心以前。这时还没握手，说话的语言照 `ui.language`，`auto` 的照系统的语言（和握手时核心算的同一个函数，
//! `UiSettings::language_for`）。

use std::path::Path;
use std::sync::Arc;

use miyu_config::Values;
use miyu_config::secret::Secret;
use miyu_core::settings::OnebotSettings;
use miyu_endpoint::config::{Config, Environment};
use miyu_endpoint::settings::UiSettings;
use miyu_store::root::DataRoot;

/// 桥要的三项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// NapCat 反连进来的端口，只听 `127.0.0.1`；`0` 是让系统挑一个（测试用）。
    pub port: u16,
    /// WebUI 的端口，只听 `127.0.0.1`；`0` 是让系统挑一个（测试用）。
    pub web: u16,
    /// NapCat 连进来时要出示的访问令牌：取到了、没写引用、取不到三种（`/status` 照它说）。
    pub token: Token,
}

/// `onebot.token` 读出来的样子（`onebot.md` 第二条 `/status` 的 `token`，施工 O-16 补二）。只有取到了的能让 NapCat 进来。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    /// 取到了：NapCat 要出示的值。`Debug` 只印 `Secret(…)`。
    Set(Secret),
    /// 没写引用（`/status` 说 `none`）。
    Unset,
    /// 写了引用，取不到：引用的密钥没存、环境变量没设（`/status` 说 `missing`）。
    Missing,
}

impl Token {
    /// 取到了的值；没有的是空的。
    pub fn secret(&self) -> Option<&Secret> {
        match self {
            Token::Set(secret) => Some(secret),
            Token::Unset | Token::Missing => None,
        }
    }
}

/// 读配置时就起不来的。令牌没设不在这里：照样起来（模块的说明）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unready {
    /// `onebot.listen` 不是 1024 到 65535 的整数。清单给了默认值、读的时候查过范围，正常走不到这里：真走到了照实说，
    /// 不悄悄换一个端口。
    BadPort,
    /// `onebot.web` 不是 1024 到 65535 的整数：同上。
    BadWebPort,
}

/// 重读一次配置（`crate::current` 用）：照起来时的数据根、家目录、系统的语言和环境读，交回三项或者为什么起不来。会读文件，
/// 在阻塞线程里调。
pub type Reload = Arc<dyn Fn() -> Result<Settings, Unready> + Send + Sync>;

/// 读到的：说话的语言，和三项（或者为什么起不来）。
#[derive(Debug)]
pub struct Loaded {
    /// 握手以前说话的语言：`zh`、`en`、`ja` 之一。
    pub language: String,
    /// 三项；起不来的说为什么。
    pub settings: Result<Settings, Unready>,
}

/// 读配置以前说话的语言：`ui.language` 照默认的 `auto`，只看系统的语言 `locale`（和握手时核心算的同一个函数）。用法
/// 不对、起不来这几句说在读配置以前。
pub fn system_language(locale: Option<&str>) -> String {
    UiSettings::from(&Values::default())
        .language_for(locale)
        .to_string()
}

/// 读数据根 `root` 的配置：管理员的个人设置也读（`ui.language` 可能写在那里），`home` 是系统的家目录，`locale` 是系统的
/// 语言，`{ env = … }` 照 `environment` 取。读不进来的照核心的规矩记一条运行日志、那一层照空的算。
pub fn load(
    root: &DataRoot,
    home: Option<&Path>,
    locale: Option<&str>,
    environment: Environment,
) -> Loaded {
    let config = Config::load(
        root,
        &miyu_core::admin(),
        home,
        miyu_core::settings::items(),
        environment,
    );
    let values = config.resolved().values();
    let language = UiSettings::from(&values).language_for(locale).to_string();
    let onebot = OnebotSettings::from(&values);
    let port = onebot.listen.and_then(|port| u16::try_from(port).ok());
    let web = onebot.web.and_then(|port| u16::try_from(port).ok());
    let token = match onebot.token.as_ref() {
        None => Token::Unset,
        Some(reference) => config.secret(reference).map_or(Token::Missing, Token::Set),
    };
    let settings = match (port, web) {
        (None, _) => Err(Unready::BadPort),
        (_, None) => Err(Unready::BadWebPort),
        (Some(port), Some(web)) => Ok(Settings { port, web, token }),
    };
    Loaded { language, settings }
}
