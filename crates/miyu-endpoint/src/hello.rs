//! 握手（`docs/designs/04-核心协议.md` 第三节、第八节，第九节「先做的几样怎么写」）：协议的主版本取双方
//! 都支持的最高的；本机连接出示本机令牌（第四节）。握手以后这个连接就是管理员（`06-多用户与身份.md`
//! 第二节）。
//!
//! 施工 W-8 起凭据正好写一种（`web-module.md`「怎么走」第一条）：本机令牌 `token`、一次性码 `code`、登录令牌 `login`，
//! 或者用户名 `user` 加密码 `password`。用一次性码的回应多 `"setup": true`，用密码的多一个登录令牌 `login`。写了不止一种、
//! 用户名和密码只写了一个：`bad_params`，回完断开。
//!
//! 回应带这个连接给人看的字用哪种语言 `language`，和配置里有几处错误 `config_errors`（施工 8-2，`config.md`
//! 「协议」）：语言照 `ui.language` 的最终值，是 `auto` 的照头报的系统语言 `locale`。
//!
//! 施工 8-4 起 `ui.language` 能当场改：连接记着头报的系统语言（[`Shaken`]），每次说话都照这时的 `ui.language` 重算
//! （[`Shaken::now`]），不用再握手（`config.md` 第二条第 8 条）。
//!
//! 回应带 `host`（施工 W-3，`web-module.md`「四、路径」）：系统的家目录（照原样）、核心所在的平台、这个账号的
//! 工作区（换成真实的位置）。管理员的工作区核心起来时就建好了（`core.md`），这里不再建：握手不该替每一个连上来
//! 的头造目录，换不成真实的位置（还没建出来）的就照原样交回。

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::AccountId;
use miyu_sandbox::Availability;

use std::sync::Arc;

use crate::Core;
use crate::config::Config;
use crate::login::{self, Via};
use crate::refusal::{Locale, Refusal};
use crate::settings::UiSettings;

/// 核心支持的协议主版本：现在只有 1。
pub(crate) const PROTOCOL: u32 = 1;

/// `hello` 的参数。认识的几格，别的不理（同一个主版本之内只做加法，第八节）。
#[derive(Debug, Deserialize)]
struct Params {
    /// 头支持的主版本范围：`[最低, 最高]`。
    protocol: [u32; 2],
    /// 头的种类和版本。
    head: Head,
    /// 头所在系统的语言：`ui.language` 是 `auto` 时照它（施工 8-2）。
    #[serde(default)]
    locale: Option<String>,
    /// 头能做什么。
    #[serde(default)]
    caps: Caps,
    /// 本机令牌。
    #[serde(default)]
    token: Option<String>,
    /// 一次性码（施工 W-8）。
    #[serde(default)]
    code: Option<String>,
    /// 登录令牌（施工 W-8）。
    #[serde(default)]
    login: Option<String>,
    /// 用户名（施工 W-8）。
    #[serde(default)]
    user: Option<String>,
    /// 密码（施工 W-8）。
    #[serde(default)]
    password: Option<String>,
}

/// 头的种类和版本。
#[derive(Debug, Deserialize)]
struct Head {
    kind: String,
    version: String,
}

/// 头能做什么：现在只看它能不能让人输入。
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Caps {
    /// 能让人输入：确认、提问有人答。
    input: bool,
}

/// 握手时记下的：头报的系统语言照 `auto` 算出的那一种，能不能让人输入。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Shaken {
    /// `ui.language` 是 `auto` 时用哪一种：头报的 `locale` 照 [`UiSettings::language_for`] 算的。
    system: &'static str,
    /// 能让人输入。
    input: bool,
}

impl Shaken {
    /// 这一刻的这个连接：语言照现在的 `ui.language` 重算。
    pub(crate) fn now(self, core: &Core) -> Peer {
        let language = language_of(&core.config(), self.system);
        Peer {
            locale: Locale::of(Some(language)),
            language,
            input: self.input,
        }
    }

    /// `ui.language` 是 `auto` 时用哪一种。
    pub(crate) fn system(self) -> &'static str {
        self.system
    }
}

/// 照配置 `config` 的 `ui.language`（不算项目配置），`auto` 的用 `system`：给人看的字用哪种语言。
pub(crate) fn language_of(config: &Config, system: &'static str) -> &'static str {
    let ui = UiSettings::from(&config.resolved().values());
    match ui.language_for(Some(system)) {
        "zh" => "zh",
        "ja" => "ja",
        _ => "en",
    }
}

/// 这个连接是谁（施工 O-4 下、O-13 上）：账号，和核心拉起的扩展是哪个包（本机的头没有）。
#[derive(Debug, Clone)]
pub(crate) struct Caller {
    /// 声明了系统账号的包的扩展是那个系统账号，别的是管理员。
    pub(crate) account: AccountId,
    /// 核心拉起的扩展是哪个包：`events.append` 的 `ext.<包>.` 照它查、记成这个模块写的。
    pub(crate) package: Option<String>,
    /// 核心往这个连接发请求的一头（施工 O-2 上）：`provide` 照它记下这个包由谁提供。
    pub(crate) reverse: crate::reverse::Peer,
}

/// 这一刻的这个连接。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Peer {
    /// 核心拒绝时的话用哪种语言：只有中文、英文，`ja` 的照英文。
    pub(crate) locale: Locale,
    /// 给人看的字用哪种语言：`zh`、`en`、`ja` 之一（施工 8-2）。配置的名字、说明、报错的话照它。
    pub(crate) language: &'static str,
    /// 能让人输入：它造的会话有人确认。
    pub(crate) input: bool,
}

/// 握手：交回这个连接记下的、它是怎么认出来的（施工 W-8）和回应。拒绝的，交回拒绝和要不要断开。`spawned` 的是核心亲手拉起的
/// 扩展（施工 9-4 上）：不看凭据，写了的也不看。`account` 是这个连接是谁（施工 O-4 下：声明了系统账号的包的扩展是它）。
pub(crate) async fn hello(
    core: &Arc<Core>,
    params: Value,
    spawned: bool,
    account: &AccountId,
) -> Result<(Shaken, Via, Value), (Refusal, bool)> {
    let params: Params =
        serde_json::from_value(params).map_err(|_| (Refusal::BAD_PARAMS, false))?;
    let [low, high] = params.protocol;
    if !(low <= PROTOCOL && PROTOCOL <= high) {
        tracing::warn!(
            target: "miyu::endpoint",
            head = params.head.kind.as_str(),
            low,
            high,
            "protocol mismatch"
        );
        return Err((Refusal::PROTOCOL, true));
    }
    let head = params.head.kind.as_str();
    let checked = match spawned {
        true => Ok((Via::Spawned, None)),
        false => credentials(core, &params).await,
    };
    let (via, login) = checked.map_err(|(refusal, said)| {
        if let Some(said) = said {
            tracing::warn!(target: "miyu::endpoint", head, "{said}");
        }
        (refusal, true)
    })?;
    tracing::info!(
        target: "miyu::endpoint",
        head,
        version = params.head.version.as_str(),
        protocol = PROTOCOL,
        via = via.label(),
        "connected"
    );
    let shaken = Shaken {
        system: system(params.locale.as_deref()),
        input: params.caps.input,
    };
    let language = shaken.now(core).language;
    let mut result = json!({
        "protocol": PROTOCOL,
        "core": {"version": env!("CARGO_PKG_VERSION")},
        "account": account.as_str(),
        "host": host(core, account),
        "sandbox": sandbox(&core.sandbox),
        "language": language,
    });
    let errors = core.config().errors();
    if errors > 0 {
        result["config_errors"] = json!(errors);
    }
    if via == Via::Code {
        result["setup"] = json!(true);
    }
    if let Some(login) = login {
        result["login"] = login;
    }
    Ok((shaken, via, result))
}

/// 照写的凭据认这个连接（施工 W-8）：交回它是怎么认出来的、用密码登录的另交登录令牌。不认的交回拒绝和运行日志里记哪一句
/// （`bad_params` 不记）。
async fn credentials(
    core: &Arc<Core>,
    params: &Params,
) -> Result<(Via, Option<Value>), (Refusal, Option<&'static str>)> {
    let written = [
        params.token.is_some(),
        params.code.is_some(),
        params.login.is_some(),
        params.user.is_some() || params.password.is_some(),
    ];
    if written.iter().filter(|written| **written).count() > 1 {
        return Err((Refusal::BAD_PARAMS, None));
    }
    if let Some(token) = &params.token {
        return match same(token, &core.token) {
            true => Ok((Via::Token, None)),
            false => Err((Refusal::BAD_TOKEN, Some("bad token"))),
        };
    }
    if let Some(code) = &params.code {
        return match core.identity.take_code(code) {
            true => Ok((Via::Code, None)),
            false => Err((Refusal::BAD_CODE, Some("bad code"))),
        };
    }
    if let Some(token) = &params.login {
        return login::login(core, token)
            .map(|via| (via, None))
            .ok_or((Refusal::BAD_LOGIN, Some("bad login")));
    }
    match (&params.user, &params.password) {
        (None, None) => Err((Refusal::BAD_TOKEN, Some("bad token"))),
        (Some(user), Some(password)) => login::password(core, user, password.clone())
            .await
            .map(|(issued, via)| (via, Some(issued)))
            .map_err(|refusal| {
                let said = match refusal.reason {
                    "login_throttled" => Some("login throttled"),
                    "bad_password" => Some("bad password"),
                    _ => None,
                };
                (refusal, said)
            }),
        _ => Err((Refusal::BAD_PARAMS, None)),
    }
}

/// 头报的系统语言 `locale` 照 `auto` 算出的那一种（`config.md` 第二条第 8 条）：`zh` 开头的是 `zh`，`ja` 开头的是 `ja`，
/// 别的、没报的是 `en`。
fn system(locale: Option<&str>) -> &'static str {
    let auto = UiSettings {
        language: "auto".to_string(),
        startup: "new".to_string(),
        head: "tui".to_string(),
        welcomed: false,
    };
    match auto.language_for(locale) {
        "zh" => "zh",
        "ja" => "ja",
        _ => "en",
    }
}

/// 握手的回应里的 `host`（施工 W-3，`web-module.md`「四、路径」）：`home` 是核心起来时拿到的系统的家目录，照
/// 原样，读不出来的是 `null`；`platform` 是核心所在的平台；`workspace` 是这个账号的工作区，换成真实的位置。不在
/// 这里建它：管理员、系统账号的工作区核心起来时就建好了（`core.md`，施工 O-4 下），握手不该替每一个连上来的头造目录；换不成
/// 的（没建出来）照原样交回，工具自己用到时会报错。
fn host(core: &Core, account: &AccountId) -> Value {
    let workspace = core.root.workspace(account);
    let workspace = std::fs::canonicalize(&workspace).unwrap_or(workspace);
    json!({
        "home": core.home.as_deref().map(|home| home.display().to_string()),
        "platform": std::env::consts::OS,
        "workspace": workspace.display().to_string(),
    })
}

/// 握手的回应里的 `sandbox`：能用的 `{"usable": true}`，用不了的带原因（施工 5-4 下）。
fn sandbox(availability: &Availability) -> Value {
    match availability {
        Availability::Usable(_) => json!({ "usable": true }),
        Availability::Unusable(reason) => json!({ "usable": false, "reason": reason.code() }),
    }
}

/// 两份令牌一样不一样：每个字节都比，比到哪一个不一样都用一样长的时间。
fn same(given: &str, expected: &str) -> bool {
    given.len() == expected.len()
        && given
            .bytes()
            .zip(expected.bytes())
            .fold(0u8, |differ, (a, b)| differ | (a ^ b))
            == 0
}
