//! 一行怎么写（`docs/designs/28-运行日志.md` 第二节，LG1）：
//!
//! ```text
//! 2026-09-27 21:03:15.284 INFO  core     started version=0.0.0 root=~/.miyu
//! 2026-09-27 21:03:18.410 DEBUG http     0199d1e6-… sent host=api.deepseek.com bytes=2396
//! ```
//!
//! 时刻、级别（占 5 格）、来源（占 8 格）、会话编号（有的才写）、这件事、键值。值里有空白、引号、
//! 等号的加双引号，引号和反斜杠用反斜杠转义；一行里不出现换行，写成 `\n`；别的控制字符写成 `\x1b`
//! 这样，`cat` 日志的时候终端不会把它当成指令。

/// 一行的几样。
pub(crate) struct Parts<'a> {
    pub(crate) time: &'a str,
    pub(crate) level: &'a tracing::Level,
    pub(crate) source: &'a str,
    pub(crate) session: Option<&'a str>,
    pub(crate) message: &'a str,
    pub(crate) fields: &'a [(&'static str, String)],
}

/// 写成一行，末尾不带换行。
pub(crate) fn format(parts: &Parts<'_>) -> String {
    let level = parts.level.to_string();
    let mut line = format!("{} {level:<5} {:<8} ", parts.time, parts.source);
    if let Some(session) = parts.session {
        line.push_str(&escape(session));
        line.push(' ');
    }
    line.push_str(&escape(parts.message));
    for (key, value) in parts.fields {
        line.push(' ');
        line.push_str(key);
        line.push('=');
        line.push_str(&quote(value));
    }
    line
}

/// 事件的目标写成来源：自己的 `miyu::http` 写成 `http`，别人家的照原样。
pub(crate) fn source(target: &str) -> &str {
    target.strip_prefix("miyu::").unwrap_or(target)
}

/// 本机时间，到毫秒。
pub(crate) fn now() -> String {
    jiff::Zoned::now()
        .strftime("%Y-%m-%d %H:%M:%S%.3f")
        .to_string()
}

/// 本机现在和 UTC 差多少：`+09:00` 这样。核心起来那一行的 `tz`（`28-运行日志.md` 第二节，施工 4-9 再补四上）。
pub fn utc_offset() -> String {
    jiff::Zoned::now().strftime("%:z").to_string()
}

/// 一行里不出现换行和别的控制字符。
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        push_escaped(&mut escaped, c);
    }
    escaped
}

/// 换行、回车、制表写成 `\n`、`\r`、`\t`，别的控制字符写成 `\x1b` 这样，其余的照原样。
fn push_escaped(out: &mut String, c: char) {
    match c {
        '\n' => out.push_str("\\n"),
        '\r' => out.push_str("\\r"),
        '\t' => out.push_str("\\t"),
        c if c.is_control() => out.push_str(&format!("\\x{:02x}", u32::from(c))),
        c => out.push(c),
    }
}

/// 值：有空白、控制字符、引号、等号的，或者是空的，加双引号，里面的反斜杠、引号、控制字符转义。
fn quote(value: &str) -> String {
    let plain = !value.is_empty()
        && !value
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || c == '"' || c == '=');
    if plain {
        return value.to_string();
    }
    let mut quoted = String::from("\"");
    for c in value.chars() {
        match c {
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\\""),
            c => push_escaped(&mut quoted, c),
        }
    }
    quoted.push('"');
    quoted
}

#[cfg(test)]
mod tests;
