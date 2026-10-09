//! 跑着的 `miyu-embed`（施工 R-5 中，`docs/blueprint/recall.md` 第四条第 4 款）：拉起、等它说 `ready`、一条一条问、关掉。
//!
//! 话怎么说在 [`Conversation`]：对着任意一对读写的两头，单元测试拿内存里的管道替它（不回话的、乱回的）。一条一条地问：问一条、
//! 等它回这一条，回的编号对不上、读不懂、过了时限的，当它坏了，交给拉起它的一方杀掉。

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

/// 等 `ready` 最多多久：载入模型平常一两百毫秒，机器忙、盘慢时留足。
const READY: Duration = Duration::from_secs(30);

/// 一条最多等多久：一百来个字平常十几毫秒。
const ANSWER: Duration = Duration::from_secs(10);

/// 关了标准输入以后等它退出最多多久，过了杀掉（照扩展的 `grace`）。
const GRACE: Duration = Duration::from_secs(5);

/// Windows 上拉起它不弹黑窗口（和扩展、`shell` 的一样）。
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 一条没问成。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Failure {
    /// 它照常回了一句错：这一条算不出，它还好着，接着用。
    Refused(String),
    /// 它坏了：退出了、乱回、过了时限。要杀掉，下一条再拉起。
    Broken(String),
}

/// 和 `miyu-embed` 说话：一行 JSON 进，一行 JSON 出。
pub(super) struct Conversation<R, W> {
    output: R,
    input: W,
    /// 下一条的编号：回的编号对得上才收。
    next: u64,
}

impl<R: AsyncBufRead + Unpin, W: AsyncWrite + Unpin> Conversation<R, W> {
    /// 对着 `output`（它的标准输出）、`input`（它的标准输入）。
    pub(super) fn new(output: R, input: W) -> Conversation<R, W> {
        Conversation {
            output,
            input,
            next: 1,
        }
    }

    /// 等它说 `ready`，最多等 `wait`：交回它报的模型编号。
    pub(super) async fn ready(&mut self, wait: Duration) -> Result<String, String> {
        let line = match tokio::time::timeout(wait, self.line()).await {
            Ok(line) => line?,
            Err(_) => return Err(format!("not ready after {} seconds", wait.as_secs_f64())),
        };
        if let Some(model) = line["ready"]["model"].as_str() {
            return Ok(model.to_string());
        }
        match line["error"].as_str() {
            Some(error) => Err(error.to_string()),
            None => Err(format!("unexpected first line: {line}")),
        }
    }

    /// 问 `text` 的向量，最多等 `wait`。
    pub(super) async fn ask(&mut self, text: &str, wait: Duration) -> Result<Vec<f32>, Failure> {
        let id = self.next.to_string();
        self.next += 1;
        let request = format!("{}\n", json!({"id": id, "text": text}));
        let asked = async {
            self.input
                .write_all(request.as_bytes())
                .await
                .map_err(|error| error.to_string())?;
            self.input
                .flush()
                .await
                .map_err(|error| error.to_string())?;
            self.line().await
        };
        let reply = match tokio::time::timeout(wait, asked).await {
            Ok(reply) => reply.map_err(Failure::Broken)?,
            Err(_) => {
                return Err(Failure::Broken(format!(
                    "no answer after {} seconds",
                    wait.as_secs_f64()
                )));
            }
        };
        if reply["id"].as_str() != Some(id.as_str()) {
            return Err(Failure::Broken(format!(
                "answer to another request: {reply}"
            )));
        }
        if let Some(error) = reply["error"].as_str() {
            return Err(Failure::Refused(error.to_string()));
        }
        let vector = reply["vector"].as_array().and_then(|values| {
            values
                .iter()
                .map(|value| value.as_f64().map(|value| value as f32))
                .collect::<Option<Vec<f32>>>()
        });
        vector.ok_or_else(|| Failure::Broken(format!("no vector in {reply}")))
    }

    /// 读一行，读成 JSON。读到头了（它退出了）、读不懂的是错。
    async fn line(&mut self) -> Result<Value, String> {
        let mut line = String::new();
        let read = self
            .output
            .read_line(&mut line)
            .await
            .map_err(|error| error.to_string())?;
        if read == 0 {
            return Err("miyu-embed exited".to_string());
        }
        serde_json::from_str(&line).map_err(|error| format!("unreadable line: {error}"))
    }
}

/// 一个拉起着的 `miyu-embed`。核心丢下它（[`Worker`] 被放下）时杀掉：`kill_on_drop`；核心整个没了，Windows 上作业对象收掉它，
/// Unix 上它读到标准输入关了自己退出。
pub(super) struct Worker {
    child: Child,
    talk: Conversation<BufReader<ChildStdout>, ChildStdin>,
}

impl std::fmt::Debug for Worker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Worker")
            .field("pid", &self.child.id())
            .finish_non_exhaustive()
    }
}

impl Worker {
    /// 拉起 `program`（照清单 `manifest`、模型目录 `dir`），等它说 `ready`：交回它和它报的模型编号。标准错误接核心的：它只在
    /// 标准输出也写不了时才往那里写。
    pub(super) async fn start(
        program: &Path,
        manifest: &Path,
        dir: &Path,
    ) -> Result<(Worker, String), String> {
        let mut command = std::process::Command::new(program);
        command
            .arg("--manifest")
            .arg(manifest)
            .arg("--dir")
            .arg(dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut command = Command::from(command);
        command.kill_on_drop(true);
        let mut child = command
            .spawn()
            .map_err(|error| format!("cannot start miyu-embed: {error}"))?;
        let (Some(input), Some(output)) = (child.stdin.take(), child.stdout.take()) else {
            return Err("miyu-embed has no pipes".to_string());
        };
        let mut talk = Conversation::new(BufReader::new(output), input);
        let model = talk.ready(READY).await?;
        Ok((Worker { child, talk }, model))
    }

    /// 问 `text` 的向量。
    pub(super) async fn ask(&mut self, text: &str) -> Result<Vec<f32>, Failure> {
        self.talk.ask(text, ANSWER).await
    }

    /// 关掉：关它的标准输入，它读完就退出；等 [`GRACE`]，没退的杀掉。
    pub(super) async fn stop(self) {
        let Worker { mut child, talk } = self;
        drop(talk);
        if tokio::time::timeout(GRACE, child.wait()).await.is_err()
            && let Err(error) = child.start_kill()
        {
            tracing::warn!(target: crate::TARGET, error = %error, "embedder not killed");
        }
    }
}

#[cfg(test)]
mod tests;
