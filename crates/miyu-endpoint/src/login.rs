//! 身份（`docs/blueprint/web-module.md`「怎么走」第一条，施工 W-8）：本机的浏览器经网页软件连核心，凭据由核心在握手时验。
//!
//! - 一次性码（`account.setup_code`）：只给出示本机令牌连上的连接；5 分钟、一次；只在内存里，最多 16 个。
//! - 用一次性码连上的连接只能设用户名和密码（`account.setup`）：设好了写 `system/accounts.json`，这个账号以前的登录令牌
//!   全部作废，造一个新的交回；这个连接从此是完整的管理员。
//! - 用户名、密码握手：对上了造一个登录令牌交回；同一个用户名 60 秒内错 5 次，剩下的时间里先拒、不验。
//! - 登录令牌握手：哈希在这个账号的 `logins.json` 里、没过期。
//! - `account.logout`：作废一个、全部；用登录令牌、密码连着的连接收到作废就断开（[`Identity::revoked`]）。
//!
//! 一次性码、密码、登录令牌一个字都不进运行日志。凭据文件的读写在 `miyu-store`（`accounts.rs`、`logins.rs`），这里排着队
//! 一件件改（[`Identity`] 的锁），argon2 放进阻塞线程。

mod files;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::broadcast;

use miyu_kernel::id::AccountId;
use miyu_kernel::origin::{By, Person};
use miyu_kernel::time::Timestamp;
use miyu_store::accounts;

use crate::Core;
use crate::refusal::Refusal;

/// 运行日志的目标。
const TARGET: &str = "miyu::endpoint";

/// 一次性码多久有效（设计 21 X6）。
pub(crate) const CODE_TTL: Duration = Duration::from_secs(5 * 60);

/// 同时最多几个一次性码：多了丢最早的。
const MOST_CODES: usize = 16;

/// 登录令牌多久有效（设计 06 U4）。
const LOGIN_DAYS: i64 = 30;

/// 数登录失败的窗口、窗口里错几次就先拒（设计 06 U4 的初值）。
const WINDOW: Duration = Duration::from_secs(60);
const TRIES: usize = 5;

/// 密码最短、最长几个字节（「起草时定的」第 45 条）。
const SHORTEST: usize = 8;
const LONGEST: usize = 1024;

/// 这个连接是怎么认出来的（握手时定）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Via {
    /// 本机令牌：本机的头。
    Token,
    /// 一次性码：只能设密码。
    Code,
    /// 登录令牌：带着它的哈希。
    Login(String),
    /// 用户名、密码：带着这次造的登录令牌的哈希。
    Password(String),
}

impl Via {
    /// 运行日志里的写法。
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Via::Token => "token",
            Via::Code => "code",
            Via::Login(_) => "login",
            Via::Password(_) => "password",
        }
    }

    /// 它靠的那个登录令牌的哈希：用登录令牌、密码连上的才有。
    pub(crate) fn login(&self) -> Option<&str> {
        match self {
            Via::Login(hash) | Via::Password(hash) => Some(hash),
            Via::Token | Via::Code => None,
        }
    }
}

/// 作废了哪些登录令牌：连着的连接照它断开。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Revoked {
    /// 这个账号全部的。
    All,
    /// 哈希是它的那一个。
    One(String),
}

/// 一个还没用的一次性码。
struct Code {
    value: String,
    expires: Instant,
}

/// 核心一份的：一次性码、登录失败的计数、作废的广播，改凭据文件排队的锁。
pub(crate) struct Identity {
    codes: Mutex<Vec<Code>>,
    failures: Mutex<BTreeMap<String, Vec<Instant>>>,
    revoked: broadcast::Sender<Revoked>,
    files: tokio::sync::Mutex<()>,
    code_ttl: Duration,
}

impl Identity {
    /// 一次性码 `code_ttl` 有效。
    pub(crate) fn new(code_ttl: Duration) -> Identity {
        Identity {
            codes: Mutex::new(Vec::new()),
            failures: Mutex::new(BTreeMap::new()),
            revoked: broadcast::channel(64).0,
            files: tokio::sync::Mutex::new(()),
            code_ttl,
        }
    }

    /// 收作废的：用登录令牌、密码连上的连接一个一份。
    pub(crate) fn revoked(&self) -> broadcast::Receiver<Revoked> {
        self.revoked.subscribe()
    }

    /// 用掉一次性码 `code`：在、没过期的交回真，当场作废。
    pub(crate) fn take_code(&self, code: &str) -> bool {
        let mut codes = self.codes.lock().unwrap_or_else(PoisonError::into_inner);
        let now = Instant::now();
        codes.retain(|each| each.expires > now);
        let before = codes.len();
        codes.retain(|each| each.value != code);
        codes.len() < before
    }
}

/// `account.setup_code`：只给出示本机令牌的连接。
pub(crate) fn setup_code(core: &Core, via: &Via) -> Result<Value, Refusal> {
    if *via != Via::Token {
        return Err(Refusal::LOCAL_ONLY);
    }
    let code = random_hex()?;
    let identity = &core.identity;
    let expires = crate::sessions::now().unix_millis() + millis(identity.code_ttl);
    {
        let mut codes = identity
            .codes
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let now = Instant::now();
        codes.retain(|each| each.expires > now);
        codes.push(Code {
            value: code.clone(),
            expires: now + identity.code_ttl,
        });
        while codes.len() > MOST_CODES {
            codes.remove(0);
        }
    }
    let first = files::read_accounts(core)
        .accounts
        .of(core.admin.as_str())
        .is_none();
    Ok(json!({"code": code, "expires": stamp(expires), "first": first}))
}

/// `account.setup` 的参数。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetupParams {
    username: String,
    password: String,
}

/// `account.setup`：用一次性码连上的设用户名、密码。交回回应和这个连接以后怎么算。
pub(crate) async fn setup(
    core: &Arc<Core>,
    via: &Via,
    params: Value,
) -> Result<(Value, Via), Refusal> {
    if *via != Via::Code {
        return Err(Refusal::BAD_PARAMS);
    }
    let params: SetupParams = serde_json::from_value(params).map_err(|_| Refusal::BAD_PARAMS)?;
    if AccountId::parse(&params.username).is_err() || !good_password(&params.password) {
        return Err(Refusal::BAD_PARAMS);
    }
    let password = params.password;
    let hashed = tokio::task::spawn_blocking(move || accounts::hash(&password))
        .await
        .map_err(|_| Refusal::INTERNAL)?
        .map_err(|error| {
            tracing::warn!(target: TARGET, error = %error, "password not hashed");
            Refusal::INTERNAL
        })?;
    let _queue = core.identity.files.lock().await;
    let now = crate::sessions::now();
    let first = files::put_account(core, &params.username, hashed, now)?;
    tracing::info!(target: TARGET, first, "password set");
    let revoked = files::revoke(core, None)?;
    if revoked > 0 {
        tracing::info!(target: TARGET, count = revoked, "logins revoked");
    }
    // 连着的旧登录一起断开（重设的时候把以前的浏览器踢出去）。
    announce(&core.identity, Revoked::All);
    let (token, hash, expires) = files::issue(core, now)?;
    tracing::info!(target: TARGET, via = "setup", "login issued");
    files::journal(core, &params.username, first, now);
    let reply = json!({
        "username": params.username,
        "login": {"expires": expires, "token": token},
    });
    Ok((reply, Via::Login(hash)))
}

/// 用户名 `user`、密码 `password` 握手：对上了交回登录令牌、它的过期时刻和这个连接怎么算。
pub(crate) async fn password(
    core: &Arc<Core>,
    user: &str,
    password: String,
) -> Result<(Value, Via), Refusal> {
    if let Some(wait) = throttled(&core.identity, user) {
        return Err(Refusal::login_throttled(millis(wait).max(1).unsigned_abs()));
    }
    let phc = files::read_accounts(core)
        .accounts
        .find(user)
        .map(|account| account.password.clone());
    let right = tokio::task::spawn_blocking(move || match phc {
        Some(phc) => accounts::verify(&phc, &password),
        // 没有这个用户名也验一次，和密码不对花一样的时间。
        None => {
            let _ = accounts::verify(stand_in(), &password);
            false
        }
    })
    .await
    .unwrap_or(false);
    if !right {
        failed(&core.identity, user);
        return Err(Refusal::BAD_PASSWORD);
    }
    core.identity
        .failures
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(user);
    let _queue = core.identity.files.lock().await;
    let (token, hash, expires) = files::issue(core, crate::sessions::now())?;
    tracing::info!(target: TARGET, via = "password", "login issued");
    Ok((
        json!({"expires": expires, "token": token}),
        Via::Password(hash),
    ))
}

/// 登录令牌 `token` 握手：认得的交回这个连接怎么算。
pub(crate) fn login(core: &Core, token: &str) -> Option<Via> {
    let hash = miyu_store::logins::digest(token);
    files::read_logins(core)
        .logins
        .valid(&hash, crate::sessions::now())
        .then_some(Via::Login(hash))
}

/// `account.logout` 的参数。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LogoutParams {
    #[serde(default)]
    all: bool,
}

/// `account.logout`：交回回应，和这个连接回完要不要断开。
pub(crate) async fn logout(
    core: &Arc<Core>,
    via: &Via,
    params: Value,
) -> Result<(Value, bool), Refusal> {
    let params: LogoutParams = serde_json::from_value(params).map_err(|_| Refusal::BAD_PARAMS)?;
    let one = match (params.all, via.login()) {
        (true, _) => None,
        (false, Some(hash)) => Some(hash.to_string()),
        (false, None) => return Err(Refusal::BAD_PARAMS),
    };
    let _queue = core.identity.files.lock().await;
    let revoked = files::revoke(core, one.as_deref())?;
    tracing::info!(target: TARGET, count = revoked, "logins revoked");
    announce(
        &core.identity,
        match one {
            Some(hash) => Revoked::One(hash),
            None => Revoked::All,
        },
    );
    Ok((json!({"revoked": revoked}), via.login().is_some()))
}

/// 告诉连着的连接哪些登录令牌作废了；一个都没在听的不要紧。
fn announce(identity: &Identity, revoked: Revoked) {
    identity.revoked.send(revoked).unwrap_or(0);
}

/// 这个用户名还要等多久才能再试：60 秒内错满 5 次的，等到最早那一次满 60 秒。
fn throttled(identity: &Identity, user: &str) -> Option<Duration> {
    let mut failures = identity
        .failures
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let now = Instant::now();
    let tries = failures.get_mut(user)?;
    tries.retain(|at| now.duration_since(*at) < WINDOW);
    (tries.len() >= TRIES).then(|| WINDOW - now.duration_since(tries[tries.len() - TRIES]))
}

/// 记一次失败。
fn failed(identity: &Identity, user: &str) {
    let mut failures = identity
        .failures
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    failures
        .entry(user.to_string())
        .or_default()
        .push(Instant::now());
}

/// 密码写得合不合：8 到 1024 个字节，不能全是空白。
fn good_password(password: &str) -> bool {
    (SHORTEST..=LONGEST).contains(&password.len()) && !password.trim().is_empty()
}

/// 没有这个用户名时陪着验的一串：照同样的参数算的，谁的密码都验不过。
fn stand_in() -> &'static str {
    static STAND_IN: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    STAND_IN.get_or_init(|| accounts::hash("miyu stand-in password").unwrap_or_default())
}

/// 32 个系统给的随机字节，写成 64 位小写十六进制。
pub(crate) fn random_hex() -> Result<String, Refusal> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|error| {
        tracing::warn!(target: TARGET, error = %error, "no random bytes");
        Refusal::INTERNAL
    })?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// 时刻写成协议上的样子。
fn stamp(unix_millis: i64) -> Value {
    Timestamp::from_unix_millis(unix_millis).map_or(Value::Null, |at| json!(at))
}

/// 时长的毫秒数。
fn millis(duration: Duration) -> i64 {
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

/// 登录令牌从 `now` 起什么时候过期。
fn login_expires(now: Timestamp) -> Timestamp {
    Timestamp::from_unix_millis(now.unix_millis() + LOGIN_DAYS * 86_400_000).unwrap_or(now)
}

/// 记进系统日志的人：现在只有管理员。
fn by(core: &Core) -> By {
    By::Person(Person {
        account: core.admin.clone(),
    })
}
