//! 档案里另配的头（`docs/blueprint/models.md`「怎么走」第一条第 4 条、第八条第 1 条，施工 8-14）：值是模板，只认
//! `{session_digest}` 一个字段——种子的 SHA-256 写成十六进制的前 26 位。种子是会话编号（同一个会话重启以后还是它），一次性
//! 调用是用途，`provider.test` 是固定的 `provider.test`。别的 `{…}` 读档案时就报错。
//!
//! 换好的头由路由挂到端点上（`miyu-session` 的 `route/choice.rs`、`route/probe.rs`），HTTP 执行器照端点另配的头发。

use sha2::{Digest, Sha256};

/// 模板里认得的那一个字段。
pub const SESSION_DIGEST: &str = "{session_digest}";

/// `provider.test` 用的种子：不属于哪个会话，固定一个。
pub const PROBE_SEED: &str = "provider.test";

/// 种子 `seed` 的 SHA-256 写成十六进制的前 26 位。
pub fn digest(seed: &str) -> String {
    let hash = Sha256::digest(seed.as_bytes());
    let hex: String = hash.iter().map(|byte| format!("{byte:02x}")).collect();
    hex[..26].to_string()
}

/// 模板 `template` 写得对不对：去掉认得的字段以后不能再有 `{`、`}`。不对的交回原话（不带值以外的东西）。
///
/// # Errors
///
/// 写了别的字段、括号没配对。
pub fn check(template: &str) -> Result<(), String> {
    match template.replace(SESSION_DIGEST, "").contains(['{', '}']) {
        true => Err(format!(
            "only {SESSION_DIGEST} can be filled in, found {template:?}"
        )),
        false => Ok(()),
    }
}

/// 照种子 `seed` 换好模板 `template`。
pub fn fill(template: &str, seed: &str) -> String {
    match template.contains(SESSION_DIGEST) {
        true => template.replace(SESSION_DIGEST, &digest(seed)),
        false => template.to_string(),
    }
}

#[cfg(test)]
mod tests;
