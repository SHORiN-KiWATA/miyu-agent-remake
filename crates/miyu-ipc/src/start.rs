//! 头连核心，没在跑就拉起来（`docs/designs/12-进程形态与分发.md` 第二节「拉起时的握手」，施工 3-9 上）。
//!
//! 先连；连不上，拿 `run/spawn.lock` 这把拉起锁，免得几个头同时拉起好几个核心；拿到以后再连一次，
//! 别的头可能刚拉起来；还是连不上，拉起核心，把一根管道的写端给它当标准输出，等它写来的那一行，不轮询。
//! 真正保证一个数据根只跑一个核心的是核心自己拿的 `run/core.lock`（施工 3-8 下），拉起锁只是免得白拉。

use std::fs::{File, OpenOptions, TryLockError};
use std::io::{self, BufRead, BufReader, PipeReader, Read};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use miyu_store::root::DataRoot;

use crate::error::{ConnectError, StartError};
use crate::listener::Connection;
use crate::ready::Ready;
use crate::{connect, connect_bare, sys};

/// 拉起锁的名字，在 `run/` 里。
const SPAWN_LOCK: &str = "spawn.lock";

/// 等拉起锁、等核心说好了，各最多等多久。
const WAIT: Duration = Duration::from_secs(10);

/// 等拉起锁、等已经在跑的核心能连上，每次歇多久。
const PAUSE: Duration = Duration::from_millis(50);

/// 核心那一行最长多少字节：原因写得再长也只读这么多。
const LINE_LIMIT: u64 = 4096;

/// 连核心，没在跑就拉起来。数据根要已经建好骨架。`start` 给出启动核心的命令，例如主程序自己加上 `core`；
/// 标准输入输出、跟终端脱开，都由这里接。
///
/// 要在 tokio 运行时里调。
///
/// # Errors
///
/// 核心起不来（它说了原因的、没说就退了的、等太久的）；别的头拉了太久；拉不起来；连不上。
pub async fn connect_or_start(
    root: &DataRoot,
    start: impl FnOnce() -> Command,
) -> Result<(Connection, String), StartError> {
    start_with(root, start, connect).await
}

/// 同 [`connect_or_start`]，只是不读本机令牌（施工 W-8，[`connect_bare`]）：网页软件转发浏览器的连接用它。
///
/// # Errors
///
/// 同 [`connect_or_start`]。
pub async fn connect_or_start_bare(
    root: &DataRoot,
    start: impl FnOnce() -> Command,
) -> Result<Connection, StartError> {
    start_with(root, start, connect_bare).await
}

/// 照 `connect` 连（读不读本机令牌由它定），没在跑就拉起来。
async fn start_with<'a, T, F, Fut>(
    root: &'a DataRoot,
    start: impl FnOnce() -> Command,
    connect: F,
) -> Result<T, StartError>
where
    F: Fn(&'a DataRoot) -> Fut,
    Fut: Future<Output = Result<T, ConnectError>>,
{
    if let Some(connected) = connected(connect(root).await)? {
        return Ok(connected);
    }
    let _spawning = spawn_lock(root).await?;
    if let Some(connected) = connected(connect(root).await)? {
        return Ok(connected);
    }
    match launch(start(), root).await? {
        Ready::Ready => connect(root).await.map_err(StartError::Connect),
        Ready::Running => until_connected(root, connect).await,
        Ready::Failed(reason) => Err(StartError::Refused(reason)),
    }
}

/// 连上了的交回；核心没在跑的是 `None`；别的错照原样。
fn connected<T>(result: Result<T, ConnectError>) -> Result<Option<T>, StartError> {
    match result {
        Ok(connected) => Ok(Some(connected)),
        Err(ConnectError::NotRunning) => Ok(None),
        Err(error) => Err(StartError::Connect(error)),
    }
}

/// 拿拉起锁，最多等 [`WAIT`]：别的头在拉，等它拉完。
async fn spawn_lock(root: &DataRoot) -> Result<File, StartError> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.run().join(SPAWN_LOCK))?;
    let deadline = Instant::now() + WAIT;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                tokio::time::sleep(PAUSE).await;
            }
            Err(TryLockError::WouldBlock) => return Err(StartError::Busy),
            Err(TryLockError::Error(error)) => return Err(StartError::Io(error)),
        }
    }
}

/// 拉起核心，等它写来的那一行。拉起来的核心跟终端脱开：标准输入、标准错误接空，标准输出是那根管道；
/// 工作目录是数据根，不占着头的当前目录（施工 3-9 下的真机验收里查出这一条原先没做：核心一直占着敲
/// `miyu ask` 时所在的目录，那是一块移动硬盘的话就卸不下来）。头不等它退出，另起一个线程替它收尸。
async fn launch(mut command: Command, root: &DataRoot) -> Result<Ready, StartError> {
    let (reader, writer) = io::pipe()?;
    command
        .current_dir(root.path())
        .stdin(Stdio::null())
        .stdout(writer)
        .stderr(Stdio::null());
    sys::detach(&mut command);
    let mut child = command.spawn()?;
    // 命令里还攥着写端的这一份：不放下，核心没说话就退了，这边也读不到头。
    drop(command);
    std::thread::spawn(move || {
        if let Err(error) = child.wait() {
            tracing::debug!(target: "miyu::ipc", error = %error, "core not reaped");
        }
    });
    hear(reader, WAIT).await
}

/// 等核心写来的那一行，最多等 `wait`。
async fn hear(reader: PipeReader, wait: Duration) -> Result<Ready, StartError> {
    let read = tokio::task::spawn_blocking(move || {
        let mut line = String::new();
        let read = BufReader::new(reader.take(LINE_LIMIT)).read_line(&mut line)?;
        Ok::<_, io::Error>((read > 0).then_some(line))
    });
    match tokio::time::timeout(wait, read).await {
        Err(_) => Err(StartError::Timeout),
        Ok(Err(joined)) => Err(StartError::Io(io::Error::other(joined))),
        Ok(Ok(Err(error))) => Err(StartError::Io(error)),
        Ok(Ok(Ok(None))) => Err(StartError::Silent),
        Ok(Ok(Ok(Some(line)))) => Ok(Ready::parse(&line)),
    }
}

/// 已经有一个核心在跑：它可能还没开始等连接（刚拿到锁），连上为止，最多等 [`WAIT`]。
async fn until_connected<'a, T, F, Fut>(root: &'a DataRoot, connect: F) -> Result<T, StartError>
where
    F: Fn(&'a DataRoot) -> Fut,
    Fut: Future<Output = Result<T, ConnectError>>,
{
    let deadline = Instant::now() + WAIT;
    loop {
        match connect(root).await {
            Ok(connected) => return Ok(connected),
            Err(ConnectError::NotRunning) if Instant::now() < deadline => {
                tokio::time::sleep(PAUSE).await;
            }
            Err(error) => return Err(StartError::Connect(error)),
        }
    }
}

#[cfg(test)]
mod tests;
