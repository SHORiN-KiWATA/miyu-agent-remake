//! 握手（`docs/designs/04-核心协议.md` 第三节、第八节，第九节「先做的几样怎么写」）：协议的主版本取双方
//! 都支持的最高的；本机连接出示本机令牌（第四节）。握手以后这个连接就是管理员（`06-多用户与身份.md`
//! 第二节）。
//!
//! 回应带这个连接给人看的字用哪种语言 `language`，和配置里有几处错误 `config_errors`（施工 8-2，`config.md`
//! 「协议」）：语言照 `ui.language` 的最终值，是 `auto` 的照头报的系统语言 `locale`。

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_sandbox::Availability;

use crate::Core;
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

/// 握手以后的这个连接。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Peer {
    /// 核心拒绝时的话用哪种语言：只有中文、英文，`ja` 的照英文。
    pub(crate) locale: Locale,
    /// 给人看的字用哪种语言：`zh`、`en`、`ja` 之一（施工 8-2）。配置的名字、说明、报错的话照它。
    pub(crate) language: &'static str,
    /// 能让人输入：它造的会话有人确认。
    pub(crate) input: bool,
}

/// 握手：交回这个连接的样子和回应。拒绝的，交回拒绝和要不要断开。
pub(crate) fn hello(core: &Core, params: Value) -> Result<(Peer, Value), (Refusal, bool)> {
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
    if !params
        .token
        .as_deref()
        .is_some_and(|token| same(token, &core.token))
    {
        tracing::warn!(target: "miyu::endpoint", head = params.head.kind.as_str(), "bad token");
        return Err((Refusal::BAD_TOKEN, true));
    }
    tracing::info!(
        target: "miyu::endpoint",
        head = params.head.kind.as_str(),
        version = params.head.version.as_str(),
        protocol = PROTOCOL,
        "connected"
    );
    let language = language(core, params.locale.as_deref());
    let peer = Peer {
        locale: Locale::of(Some(language)),
        language,
        input: params.caps.input,
    };
    let mut result = json!({
        "protocol": PROTOCOL,
        "core": {"version": env!("CARGO_PKG_VERSION")},
        "account": core.admin.as_str(),
        "sandbox": sandbox(&core.sandbox),
        "language": language,
    });
    let errors = core.config.errors();
    if errors > 0 {
        result["config_errors"] = json!(errors);
    }
    Ok((peer, result))
}

/// 这个连接给人看的字用哪种语言（`config.md` 第二条第 8 条）：`ui.language` 的最终值（不算项目配置）定了的就是它，
/// `auto` 的照头报的系统语言 `locale`。
fn language(core: &Core, locale: Option<&str>) -> &'static str {
    let ui = UiSettings::from(&core.config.resolved().values());
    match ui.language_for(locale) {
        "zh" => "zh",
        "ja" => "ja",
        _ => "en",
    }
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
