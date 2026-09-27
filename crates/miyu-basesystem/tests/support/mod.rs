//! 读的三件工具共用的测试场地：一个用完就删的临时目录，里面有假的家 `home/`、工作区 `work/`、数据根 `data/`。
//! 调工具时工作目录是 `work/`。

#![allow(dead_code, reason = "三份测试各用其中几样")]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use miyu_kernel::block::Block;
use miyu_tool::{Call, Done, Progress, Tool};

/// 源码树里的资源目录。
pub fn resources() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources")
}

/// 从资源目录造出来的一件工具。
pub fn tool(name: &str) -> Arc<dyn Tool> {
    let tools = miyu_basesystem::tools(&resources()).expect("资源目录里的字读得出来");
    tools
        .into_iter()
        .find(|tool| tool.spec().name == name)
        .unwrap_or_else(|| panic!("有 {name}"))
}

/// 一个用完就删的临时目录。`now` 是造它的那一刻：设修改时间都从它往回算，秒数一样的时间就一样。
pub struct Site(pub PathBuf, SystemTime);

impl Site {
    pub fn new() -> Site {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("miyu-base-{}-{n}", std::process::id()));
        for sub in ["home", "work", "data"] {
            std::fs::create_dir_all(dir.join(sub)).expect("建得了目录");
        }
        Site(dir, SystemTime::now())
    }

    /// 在场地里写一个文件，上级目录不在就建。
    pub fn file(&self, path: &str, bytes: &[u8]) {
        let path = self.0.join(path);
        std::fs::create_dir_all(path.parent().expect("有上级目录")).expect("建得了目录");
        std::fs::write(path, bytes).expect("写得进");
    }

    /// 把场地里的 `path` 的修改时间设成造场地那一刻的 `seconds` 秒以前：新的在前靠它排，不靠写文件的先后。
    pub fn aged(&self, path: &str, seconds: u64) {
        let file = std::fs::File::options()
            .write(true)
            .open(self.0.join(path))
            .expect("打得开");
        file.set_modified(self.1 - Duration::from_secs(seconds))
            .expect("设得了修改时间");
    }

    /// 场地里的一处，换成真实的位置（得存在）。
    pub fn real(&self, path: &str) -> PathBuf {
        std::fs::canonicalize(self.0.join(path)).expect("在")
    }

    /// 在 `work/` 里调一次工具 `name`，参数是 `args`：交回出没出错、给她看的字。
    pub async fn call(&self, name: &str, args: serde_json::Value) -> (bool, String) {
        self.call_in("work", name, args).await
    }

    /// 在场地里的 `cwd` 这个工作目录里调一次工具。
    pub async fn call_in(&self, cwd: &str, name: &str, args: serde_json::Value) -> (bool, String) {
        let call = Call {
            args: args.to_string(),
            cwd: self.0.join(cwd).to_string_lossy().into_owned(),
            home: Some(self.0.join("home")),
            data_root: Some(self.0.join("data")),
        };
        let Done { error, blocks } = tool(name).run(call, Progress::new(|_| {})).await;
        let text = blocks
            .iter()
            .map(|block| match block {
                Block::Text(text) => text.text.clone(),
                other => panic!("只该有字：{other:?}"),
            })
            .collect();
        (error, text)
    }
}

impl Drop for Site {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 照平台原生的分隔符写一条相对路径：测试里写 `/`，Windows 上换成 `\`。
pub fn native(path: &str) -> String {
    path.replace('/', std::path::MAIN_SEPARATOR_STR)
}

/// 一个真实的位置在工作目录外面时写给她看的样子：绝对路径，Windows 上去掉 `\\?\` 那个前缀。
pub fn absolute(real: &Path) -> String {
    let text = real.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) => rest.to_string(),
        None => text.into_owned(),
    }
}
