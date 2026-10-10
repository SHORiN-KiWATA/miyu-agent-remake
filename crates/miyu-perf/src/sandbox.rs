//! 沙盒数据根和拉起的核心（施工 V-1）：照 `cargo xtask dev-home` 的写法建骨架、写一份系统配置，模型指到假服务。
//!
//! 环境照集成测试拉核心的那一套（`crates/miyu/tests/support`）：不去 models.dev 拉目录，缓存目录放进数据根，不用
//! `$XDG_RUNTIME_DIR`，不走代理。模型的窗口写成 [`WINDOW`]（施工 V-2 上）：长会话照真用时那样到线就压，量的是压过的会话；
//! V-1 没写窗口，会话一直不压，大会话的投影是把一万条事件整份组装，真用时到不了。

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// 量尺的模型的上下文窗口（施工 V-2 上）：常见的 20 万，压缩线 17 万。
pub const WINDOW: u64 = 200_000;

use miyu_ipc::Lock;
use miyu_store::env::{Env, Platform};
use miyu_store::root::DataRoot;

/// 核心起来最多等多久。
const START_PATIENCE: Duration = Duration::from_secs(60);

/// 一个沙盒数据根，和拉起核心要的几样。
pub struct Sandbox {
    /// 数据根。
    pub root: DataRoot,
    /// 会话在哪个目录干活：数据根旁边的 `work/`。
    pub work: PathBuf,
    miyu: PathBuf,
    resources: PathBuf,
}

impl Sandbox {
    /// 在 `dir` 下建一个新的：有旧的先删掉。模型是 `base_url` 上的假服务。
    ///
    /// # Errors
    ///
    /// 删不掉旧的、建不了、写不进配置。
    pub fn new(
        dir: &Path,
        miyu: &Path,
        resources: &Path,
        base_url: &str,
    ) -> Result<Sandbox, String> {
        if dir.exists() {
            std::fs::remove_dir_all(dir)
                .map_err(|e| format!("删不掉旧的 {}：{e}", dir.display()))?;
        }
        let home = dir.join("home");
        let work = dir.join("work");
        std::fs::create_dir_all(&work).map_err(|e| format!("建不了 {}：{e}", work.display()))?;
        let root = DataRoot::locate(&Env {
            platform: Platform::current(),
            miyu_home: Some(home.into_os_string()),
            home: None,
            xdg_cache_home: None,
            local_app_data: None,
            miyu_resources: None,
            exe: None,
        })
        .map_err(|e| format!("数据根找不到：{e}"))?;
        root.prepare().map_err(|e| format!("数据根建不了：{e}"))?;
        let config = format!(
            "[providers.perf]\ndriver = \"openai-chat\"\nbase_url = \"{base_url}\"\nkey = {{ env = \"MIYU_PERF_KEY\" }}\n\n[providers.perf.models.perf-model]\nwindow = {WINDOW}\n\n[models]\nchat = \"perf/perf-model\"\n"
        );
        std::fs::write(root.system().join("config.toml"), config)
            .map_err(|e| format!("系统配置写不进：{e}"))?;
        Ok(Sandbox {
            root,
            work,
            miyu: miyu.to_path_buf(),
            resources: resources.to_path_buf(),
        })
    }

    /// 拉起核心，等它写来 `ready`：交回跑着的核心，和从拉起到 `ready` 用了多久。
    ///
    /// # Errors
    ///
    /// 拉不起来、没写 `ready` 就走了、等太久。
    pub fn start(&self) -> Result<Running, String> {
        let cache = self.root.state().join("perf-cache");
        let began = Instant::now();
        let mut child = Command::new(&self.miyu)
            .args(["core", "--idle-seconds", "3600"])
            .env("MIYU_HOME", self.root.path())
            .env("MIYU_RESOURCES", &self.resources)
            .env("MIYU_CATALOG_UPDATE", "false")
            .env("XDG_CACHE_HOME", &cache)
            .env("LOCALAPPDATA", &cache)
            .env("MIYU_PERF_KEY", "perf")
            .env("NO_PROXY", "127.0.0.1")
            .env_remove("XDG_RUNTIME_DIR")
            .env_remove("MIYU_LOG")
            .env_remove("HTTP_PROXY")
            .env_remove("HTTPS_PROXY")
            .env_remove("ALL_PROXY")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("拉不起 {}：{e}", self.miyu.display()))?;
        let stdout = child.stdout.take().ok_or("核心的标准输出没接上")?;
        // 读那一行放到别的线程：核心卡住不写也不走的时候，这里照样能按时放弃。
        let (sender, line) = mpsc::channel();
        std::thread::spawn(move || {
            let mut text = String::new();
            let read = BufReader::new(stdout).read_line(&mut text).map(|_| text);
            #[expect(clippy::let_underscore_must_use, reason = "等的一方放弃了就没人收")]
            let _ = sender.send(read);
        });
        let mut running = Running {
            child,
            ready: Duration::ZERO,
        };
        match line.recv_timeout(START_PATIENCE) {
            Ok(Ok(text)) if text == "ready\n" => {
                running.ready = began.elapsed();
                Ok(running)
            }
            outcome => {
                running.kill();
                Err(format!(
                    "核心没起来（{outcome:?}）：{}",
                    tail(&self.root.state().join("logs").join("core.log"))
                ))
            }
        }
    }

    /// 停掉核心，等它放开数据根的锁：下一次拉起量的才是干净的冷启动。
    ///
    /// # Errors
    ///
    /// 等了一分钟锁还在。
    pub fn stop(&self, mut running: Running) -> Result<(), String> {
        running.kill();
        let deadline = Instant::now() + START_PATIENCE;
        while Instant::now() < deadline {
            if Lock::acquire(&self.root).is_ok() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Err("核心停了，数据根的锁一分钟还没放开".to_string())
    }
}

/// 跑着的核心。丢掉时结束它。
pub struct Running {
    child: Child,
    /// 从拉起到写来 `ready` 用了多久。
    pub ready: Duration,
}

impl Running {
    /// 核心的进程号。
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    fn kill(&mut self) {
        #[expect(clippy::let_underscore_must_use, reason = "已经走了的结束不了，照样收")]
        let _ = self.child.kill();
        #[expect(
            clippy::let_underscore_must_use,
            reason = "收不到也只是留一个僵尸到量尺退出"
        )]
        let _ = self.child.wait();
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.kill();
    }
}

/// 运行日志的最后几行：核心起不来时附在报错后面。
fn tail(log: &Path) -> String {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(20)..].join("\n")
}
