//! 核心崩了，后台命令跟着没（施工 7-8，`docs/blueprint/core.md`「子进程随核心退出」）：真的 `miyu core`，模型是本机回环
//! 上的假服务器（照 DeepSeek 的写法回一次调 `shell` 放到后台、再说一句）。后台命令是一个心跳：一个子 shell 在后台每
//! 0.1 秒往文件里写一行（Unix 上它是命令起的孙进程，Windows 上是 PowerShell 自己）。心跳跑起来以后硬杀核心（Unix 上
//! `SIGKILL`，Windows 上结束进程，不连子进程），心跳要在几秒内停下：Unix 上组里看门的杀了整组，Windows 上核心所在的
//! 作业对象跟着关。
//!
//! 没停下的（修之前的样子），测试最后照心跳记下的进程号把它杀掉：只杀测试自己起的。

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader as AsyncReader};

use crate::support::{Home, within};
use miyu_http::testkit::{Piece, Reply, Server};

/// 一段事件流：照 DeepSeek 的写法，一块 `data:` 一行，最后 `[DONE]`。
fn stream(chunks: &[Value]) -> Reply {
    let mut body = String::new();
    for chunk in chunks {
        body.push_str(&format!("data: {chunk}\n\n"));
    }
    body.push_str("data: [DONE]\n\n");
    Reply::stream(vec![Piece::Bytes(body.into_bytes())])
}

/// 模型的两次回复：先调 `shell` 把 `command` 放到后台，再说一句。
fn replies(command: &str) -> Vec<Reply> {
    let args = json!({"command": command, "description": "心跳", "run_in_background": true});
    let call = json!({"choices": [{"index": 0, "delta": {"role": "assistant", "tool_calls": [{
        "index": 0, "id": "call_a", "type": "function",
        "function": {"name": "shell", "arguments": args.to_string()}}]}, "finish_reason": null}]});
    let done = json!({"choices": [{"index": 0, "delta": {}, "finish_reason": "tool_calls"}]});
    let said = json!({"choices": [{"index": 0, "delta": {"role": "assistant", "content": "放出去了。"},
        "finish_reason": "stop"}]});
    vec![stream(&[call, done]), stream(&[said])]
}

/// 心跳：`beat` 每 0.1 秒多一行，写心跳的进程号记在 `pid` 里。
#[cfg(unix)]
fn heartbeat(beat: &Path, pid: &Path) -> String {
    let (beat, pid) = (beat.display(), pid.display());
    format!("(while :; do echo x >> '{beat}'; sleep 0.1; done) & echo $! > '{pid}'; wait")
}

/// 同上，PowerShell。
#[cfg(windows)]
fn heartbeat(beat: &Path, pid: &Path) -> String {
    let (beat, pid) = (beat.display(), pid.display());
    format!(
        "Set-Content -Path '{pid}' -Value $PID; while ($true) {{ Add-Content -Path '{beat}' -Value x; Start-Sleep -Milliseconds 100 }}"
    )
}

/// 杀掉心跳记下的那个进程（只在它没跟着核心停下时用）。
fn kill_heartbeat(pid: &Path) {
    let Ok(pid) = std::fs::read_to_string(pid) else {
        return;
    };
    let pid = pid.trim();
    #[cfg(unix)]
    let killed = Command::new("kill").args(["-9", pid]).status();
    #[cfg(windows)]
    let killed = Command::new("taskkill")
        .args(["/F", "/T", "/PID", pid])
        .status();
    #[expect(clippy::let_underscore_must_use, reason = "收拾不掉也只是留下一个心跳")]
    let _ = killed;
}

/// 心跳文件有多大。
fn size(beat: &Path) -> u64 {
    std::fs::metadata(beat).map_or(0, |meta| meta.len())
}

/// 等到心跳一秒不再长，最多等 `limit`；交回停没停。
fn settled(beat: &Path, limit: Duration) -> bool {
    let deadline = Instant::now() + limit;
    let mut last = size(beat);
    let mut since = Instant::now();
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
        let now = size(beat);
        if now != last {
            (last, since) = (now, Instant::now());
        } else if since.elapsed() >= Duration::from_secs(1) {
            return true;
        }
    }
    false
}

/// 在连接上说 JSON-RPC：发一条，读到它的回应为止（推送跳过）。
struct Rpc {
    read: AsyncReader<tokio::io::ReadHalf<miyu_ipc::Connection>>,
    write: tokio::io::WriteHalf<miyu_ipc::Connection>,
}

impl Rpc {
    async fn call(&mut self, id: &str, method: &str, params: Value) -> Value {
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.write
            .write_all(format!("{request}\n").as_bytes())
            .await
            .expect("写得进");
        loop {
            let mut line = String::new();
            within(&format!("{method} 的回应"), self.read.read_line(&mut line))
                .await
                .expect("读得到");
            let reply: Value = serde_json::from_str(&line).expect("是 JSON");
            if reply["id"] == json!(id) {
                assert!(reply.get("error").is_none(), "{method}：{reply}");
                return reply["result"].clone();
            }
        }
    }
}

/// 拉起核心：模型是 `server`（照 `cargo xtask dev-home` 造的那样写一份系统配置，施工 8-6），等它写来 `ready`。
fn start(home: &Home, server: &Server) -> Child {
    let config = format!(
        "[providers.dev]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\ncatalog = \"deepseek\"\nkeys = [{{ env = \"MIYU_TEST_KEY\" }}]\n\n[models]\nchat = \"dev/deepseek-chat\"\n",
        server.base_url
    );
    std::fs::write(home.root.path().join("system").join("config.toml"), config).expect("写得进");
    let mut core = home
        .core()
        .env("MIYU_TEST_KEY", "test")
        .env("NO_PROXY", "127.0.0.1")
        .env_remove("HTTP_PROXY")
        .env_remove("HTTPS_PROXY")
        .env_remove("ALL_PROXY")
        .stdout(Stdio::piped())
        .spawn()
        .expect("起得来");
    let mut line = String::new();
    BufReader::new(core.stdout.take().expect("接了管道"))
        .read_line(&mut line)
        .expect("读得到那一行");
    assert_eq!(line, "ready\n", "核心起来了：{}", home.core_log());
    core
}

#[tokio::test]
async fn a_background_command_dies_with_a_killed_core() {
    let home = Home::new();
    let work: PathBuf = home.dir.join("work");
    std::fs::create_dir_all(&work).expect("建得了");
    let (beat, pid) = (work.join("beat.txt"), work.join("beat.pid"));
    let server = Server::start(replies(&heartbeat(&beat, &pid))).await;
    let mut core = start(&home, &server);

    let (connection, token) = within("连上", miyu_ipc::connect(&home.root))
        .await
        .expect("连得上");
    let (read, write) = tokio::io::split(connection);
    let mut rpc = Rpc {
        read: AsyncReader::new(read),
        write,
    };
    let hello = json!({"protocol": [1, 1], "head": {"kind": "test", "version": "0.0.0"},
        "caps": {"input": false}, "token": token});
    rpc.call("hello-1", "hello", hello).await;
    let created = rpc
        .call(
            "c1",
            "session.create",
            json!({"cwd": work.to_string_lossy()}),
        )
        .await;
    let session = created["session"].as_str().expect("有编号").to_string();
    rpc.call(
        "c2",
        "session.set_permission_level",
        json!({"session": session, "level": "full"}),
    )
    .await;
    rpc.call(
        "c3",
        "session.send",
        json!({"session": session, "text": "跑个心跳"}),
    )
    .await;

    // 心跳跑起来了：文件在长。
    let started = Instant::now();
    while size(&beat) < 3 && started.elapsed() < Duration::from_secs(60) {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(size(&beat) >= 3, "心跳没跑起来：{}", home.core_log());

    core.kill().expect("杀得了核心");
    core.wait().expect("收得了");
    let stopped = tokio::task::spawn_blocking({
        let beat = beat.clone();
        move || settled(&beat, Duration::from_secs(60))
    })
    .await
    .expect("没 panic");
    if !stopped {
        kill_heartbeat(&pid);
    }
    assert!(stopped, "核心没了，后台命令还在跑");
}
