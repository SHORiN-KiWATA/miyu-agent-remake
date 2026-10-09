//! 跑包自己的检查（施工 9-2，`docs/blueprint/packages.md`「格式」第 2 条、「怎么走」）：照核心起来时读到的清单，有
//! `[check]` 的每个包跑 `<程序> <args…>`。程序只找 `miyu` 旁边的（`miyu_store::packages::locate`），标准输入是
//! 空的、环境照核心的，最多等 [`LIMIT`]。标准输出一行一个 JSON，照 `check` 回应的格子收；退出码 0、1 是正常的。跑不起来、
//! 别的退出码、到时没完的报一条 `check_failed`，程序没找到的报 `check_unavailable`，有看不懂的行的报 `check_output`，都是
//! 警告。

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

use miyu_store::human::Human;
use miyu_store::packages::locate;

use crate::Core;

/// 一个包的检查最多等多久。
const LIMIT: Duration = Duration::from_secs(30);

/// 一个包的检查最多收多少字节的输出：多的不读。
const OUTPUT_BYTES: u64 = 1 << 20;

/// 跑一次的结果。
#[derive(Debug, PartialEq, Eq)]
enum Ran {
    /// 跑完了：退出码（被信号杀掉的没有）、标准输出。
    Exited(Option<i32>, String),
    /// 到时没完，杀掉了。
    TimedOut,
    /// 起不来、等不了：原因。
    Failed(String),
}

/// 跑每一个有 `[check]` 的包，交回它们报的和跑的时候出的问题，照编号的先后。
pub(super) async fn packages(core: &Core, words: &Human) -> Vec<Value> {
    let main = std::env::current_exe().unwrap_or_else(|_| "miyu".into());
    let mut problems = Vec::new();
    for found in core.packages().iter() {
        let Ok(manifest) = &found.read else {
            continue;
        };
        let (Some(command), Some(check)) = (&manifest.command, &manifest.check) else {
            continue;
        };
        let file = super::shown(core, &found.path);
        let Some(program) = locate(&command.program, &main) else {
            problems.push(trouble(words, &file, "check_unavailable", &command.program));
            continue;
        };
        let args = check.args.clone();
        let ran = tokio::task::spawn_blocking(move || run(&program, &args, LIMIT))
            .await
            .unwrap_or_else(|_| Ran::Failed("the check did not finish".to_string()));
        problems.extend(read(words, &file, ran));
    }
    problems
}

/// 一个包跑的结果换成几条问题。
fn read(words: &Human, file: &str, ran: Ran) -> Vec<Value> {
    let output = match ran {
        Ran::Exited(Some(0 | 1), output) => output,
        Ran::Exited(Some(code), _) => {
            return vec![trouble(
                words,
                file,
                "check_failed",
                &format!("exit code {code}"),
            )];
        }
        Ran::Exited(None, _) => {
            return vec![trouble(words, file, "check_failed", "killed by a signal")];
        }
        Ran::TimedOut => return vec![trouble(words, file, "check_failed", "timed out")],
        Ran::Failed(why) => return vec![trouble(words, file, "check_failed", &why)],
    };
    let mut problems = Vec::new();
    let mut unreadable = 0;
    for line in output.lines().filter(|line| !line.trim().is_empty()) {
        match problem(line) {
            Some(problem) => problems.push(problem),
            None => unreadable += 1,
        }
    }
    if unreadable > 0 {
        problems.push(trouble(
            words,
            file,
            "check_output",
            &unreadable.to_string(),
        ));
    }
    problems
}

/// 一行输出换成一条问题：`kind`、`file`、`message` 是字，`level` 是 `error` 或 `warning`；`line`、`column` 是正整数，`code`、
/// `key`、`rule`、`source` 是字，有的才收。别的格不收。不合的是没有。
fn problem(line: &str) -> Option<Value> {
    let value: Value = serde_json::from_str(line).ok()?;
    let object = value.as_object()?;
    let mut out = Map::new();
    for key in ["kind", "file", "message"] {
        out.insert(key.to_string(), json!(object.get(key)?.as_str()?));
    }
    let level = object.get("level")?.as_str()?;
    if !matches!(level, "error" | "warning") {
        return None;
    }
    out.insert("level".to_string(), json!(level));
    for key in ["line", "column"] {
        if let Some(value) = object.get(key) {
            out.insert(key.to_string(), json!(value.as_u64().filter(|n| *n > 0)?));
        }
    }
    for key in ["code", "key", "rule", "source"] {
        if let Some(value) = object.get(key) {
            out.insert(key.to_string(), json!(value.as_str()?));
        }
    }
    Some(Value::Object(out))
}

/// 跑的时候出的问题：一条警告，种类 `package`，文件是清单，话照 `package-problems/<code>`。
fn trouble(words: &Human, file: &str, code: &str, detail: &str) -> Value {
    let message =
        crate::packages::sentence(words, code, detail).unwrap_or_else(|| detail.to_string());
    json!({"kind": "package", "file": file, "code": code, "level": "warning", "message": message})
}

/// 跑 `program args…`：标准输入是空的，标准错误不要，标准输出最多收 [`OUTPUT_BYTES`]；最多等 `limit`，到时杀掉。
fn run(program: &Path, args: &[String], limit: Duration) -> Ran {
    let spawned = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let mut child = match spawned {
        Ok(child) => child,
        Err(error) => return Ran::Failed(error.to_string()),
    };
    let stdout = child.stdout.take();
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(stdout) = stdout
            && stdout.take(OUTPUT_BYTES).read_to_string(&mut text).is_err()
        {
            // 读不完、不是 UTF-8 的：读到哪算哪。
        }
        text
    });
    let deadline = Instant::now() + limit;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = reader.join().unwrap_or_default();
                return Ran::Exited(status.code(), output);
            }
            Ok(None) if Instant::now() >= deadline => {
                if child.kill().is_err() || child.wait().is_err() {
                    // 杀不掉、等不到的也只能这样：照到时没完报。
                }
                return Ran::TimedOut;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(error) => return Ran::Failed(error.to_string()),
        }
    }
}

#[cfg(test)]
mod tests;
