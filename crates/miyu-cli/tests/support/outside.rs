//! 数据根外面的临时目录。

use std::path::{Path, PathBuf};

use super::next;

/// 数据根外面的一个临时目录，用完就删：当项目目录，或者当工作区外面。放在 cargo 给集成测试的 `target/tmp`
/// 下面：系统的临时目录整个能读能写，放在里面就造不出「工作区外面」（施工 4-3 下）。
pub struct Outside(pub PathBuf);

impl Outside {
    pub fn new() -> Outside {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "miyu-cli-out-{}-{}",
            std::process::id(),
            next()
        ));
        std::fs::create_dir_all(&dir).expect("建得了目录");
        Outside(dir)
    }

    /// 在里面写一份文件，交回它的路径。
    pub fn file(&self, name: &str, text: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, text).expect("写得进");
        path
    }

    /// 这个目录，写成字。
    pub fn text(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }
}

impl Drop for Outside {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
