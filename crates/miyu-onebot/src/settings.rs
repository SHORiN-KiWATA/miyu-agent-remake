//! 桥起来时读的配置（`onebot.md` 第一条「对外的样子」「怎么走」第 1 条）：系统配置里的 `onebot.listen`、`onebot.token`，
//! 起来时读一次（`head_start`）。
//!
//! 两项由核心替桥声明（`miyu_core::settings::OnebotSettings`，权宜，9-1 挪进桥自己的清单）。桥照核心登记的全部清单读
//! 系统配置、个人设置、密钥文件（`miyu_endpoint::config::Config::load`，只读，同一份代码），令牌照引用取：`{ secret }` 照
//! 密钥文件，`{ env }` 照桥自己的环境。密钥从不经协议交出去（`config.md` 第九条），桥要用值，只能自己读。
//!
//! 读配置在连核心以前：令牌没设，不连核心、不开端口就走（第 1 条）。这时还没握手，说话的语言照 `ui.language`，`auto` 的
//! 照系统的语言（和握手时核心算的同一个函数，`UiSettings::language_for`）。

use std::path::Path;

use miyu_config::Values;
use miyu_config::secret::Secret;
use miyu_core::settings::OnebotSettings;
use miyu_endpoint::config::{Config, Environment};
use miyu_endpoint::settings::UiSettings;
use miyu_store::root::DataRoot;

/// 桥要的两项。
#[derive(Debug)]
pub struct Settings {
    /// NapCat 反连进来的端口，只听 `127.0.0.1`；`0` 是让系统挑一个（测试用）。
    pub port: u16,
    /// NapCat 连进来时要出示的访问令牌。`Debug` 只印 `Secret(…)`。
    pub token: Secret,
}

/// 读配置时就起不来的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unready {
    /// `onebot.token` 没设，或者引用的密钥、环境变量取不到。
    NoToken,
    /// `onebot.listen` 不是 1024 到 65535 的整数。清单给了默认值、读的时候查过范围，正常走不到这里：真走到了照实说，
    /// 不悄悄换一个端口。
    BadPort,
}

/// 读到的：说话的语言，和两项（或者为什么起不来）。
#[derive(Debug)]
pub struct Loaded {
    /// 握手以前说话的语言：`zh`、`en`、`ja` 之一。
    pub language: String,
    /// 两项；起不来的说为什么。
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
    let token = onebot
        .token
        .as_ref()
        .and_then(|reference| config.secret(reference));
    let settings = match (port, token) {
        (_, None) => Err(Unready::NoToken),
        (None, Some(_)) => Err(Unready::BadPort),
        (Some(port), Some(token)) => Ok(Settings { port, token }),
    };
    Loaded { language, settings }
}
