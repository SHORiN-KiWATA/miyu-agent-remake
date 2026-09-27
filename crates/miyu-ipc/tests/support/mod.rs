//! 几个测试共用的：临时的数据根、找套接字放哪的快照、头和核心两头各说一句。

#![allow(dead_code, reason = "几个测试各用其中一部分")]

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use miyu_ipc::{Dirs, Opened, connect};
use miyu_store::env::{Env, Platform};
use miyu_store::root::DataRoot;

/// 一个用完就删的临时目录，里面一个建好了骨架的数据根。
pub struct Home {
    pub dir: PathBuf,
    pub root: DataRoot,
}

impl Home {
    /// 数据根在 `<临时目录>/root`。
    pub fn new() -> Home {
        Home::named("root")
    }

    /// 数据根的路径很长：`run/core.sock` 放不下。
    pub fn deep() -> Home {
        Home::named(&"a".repeat(120))
    }

    fn named(name: &str) -> Home {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("miyu-ipc-{}-{n}", std::process::id()));
        fs::create_dir_all(&dir).expect("临时目录建得了");
        let root = DataRoot::locate(&Env {
            platform: Platform::current(),
            miyu_home: Some(dir.join(name).into_os_string()),
            home: None,
            xdg_cache_home: None,
            local_app_data: None,
            miyu_resources: None,
            exe: None,
        })
        .expect("MIYU_HOME 是绝对路径");
        root.prepare().expect("临时目录里建得了骨架");
        Home { dir, root }
    }

    /// 找套接字放哪的快照：不用 `$XDG_RUNTIME_DIR`，临时目录是这个临时目录下的 `t/`。
    #[cfg(unix)]
    pub fn dirs(&self) -> Dirs {
        Dirs {
            runtime_dir: None,
            temp_dir: self.private_dir("t"),
            ..Dirs::current()
        }
    }

    /// 找套接字放哪的快照：Windows 上是命名管道，只看平台。
    #[cfg(windows)]
    pub fn dirs(&self) -> Dirs {
        Dirs::current()
    }

    /// 这个临时目录下只有自己能进的目录（0700），没有的建。
    #[cfg(unix)]
    pub fn private_dir(&self, name: &str) -> PathBuf {
        self.dir_with_mode(name, 0o700)
    }

    /// 这个临时目录下权限是 `mode` 的目录，没有的建。
    #[cfg(unix)]
    pub fn dir_with_mode(&self, name: &str, mode: u32) -> PathBuf {
        let dir = self.dir.join(name);
        fs::create_dir_all(&dir).expect("建得了");
        fs::set_permissions(&dir, fs::Permissions::from_mode(mode)).expect("改得了");
        dir
    }

    /// `run/socket` 里记的位置。
    pub fn location(&self) -> PathBuf {
        let text = fs::read_to_string(self.root.run().join("socket")).expect("记下了位置");
        PathBuf::from(text.trim_end())
    }

    /// `run/token` 里的令牌。
    pub fn token(&self) -> String {
        let text = fs::read_to_string(self.root.run().join("token")).expect("有令牌");
        text.trim_end().to_string()
    }
}

impl Drop for Home {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// 权限位。
#[cfg(unix)]
pub fn mode(path: &Path) -> u32 {
    fs::metadata(path).expect("在").permissions().mode() & 0o777
}

/// 等 `future`，最多十秒。
pub async fn within<T>(what: &str, future: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(10), future)
        .await
        .unwrap_or_else(|_| panic!("十秒内没等到{what}"))
}

/// 头连上核心，两头各说一句。
pub async fn talk(home: &Home, opened: &mut Opened) {
    let (mut head, token) = within("连上", connect(&home.root)).await.expect("连得上");
    assert_eq!(token, opened.token);
    let mut core = within("接到连接", opened.listener.accept())
        .await
        .expect("接得到");
    head.write_all(b"ping").await.expect("写得进");
    let mut heard = [0u8; 4];
    within("核心读到", core.read_exact(&mut heard))
        .await
        .expect("读得到");
    assert_eq!(&heard, b"ping");
    core.write_all(b"pong").await.expect("写得进");
    within("头读到", head.read_exact(&mut heard))
        .await
        .expect("读得到");
    assert_eq!(&heard, b"pong");
}
