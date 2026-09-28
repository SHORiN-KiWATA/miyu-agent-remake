//! 几个测试共用的：一个用完就删的临时目录，里面造好假的家、工作区、数据根、临时目录和「系统目录」。

#![allow(dead_code, reason = "几个测试各用其中一部分")]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use miyu_fs::{Boundary, Places};

/// 一个用完就删的临时目录。
pub struct Scratch(pub PathBuf);

impl Scratch {
    pub fn new() -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("miyu-fs-{}-{n}", std::process::id()));
        fs::create_dir_all(&dir).expect("临时目录里建得了");
        Scratch(dir)
    }
}

impl Drop for Scratch {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 造一份文件，上级目录没有的先建。
pub fn file(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().expect("有上级目录")).expect("建得了目录");
    fs::write(path, text).expect("写得进");
}

/// 一个造好的场地：
///
/// ```text
/// home/  notes.txt  .ssh/id  .cargo/registry/x.rs
/// work/  src/a.rs  .git/hooks/pre-commit  .git/config  .git/HEAD  sub/.git/hooks/x
/// data/  run/token  home/admin/workspace/w.txt
/// tmp/   t.txt
/// sys/   lib.txt
/// work-other/  x.txt
/// ```
pub struct Site {
    pub scratch: Scratch,
}

impl Site {
    pub fn new() -> Site {
        let scratch = Scratch::new();
        let root = &scratch.0;
        for (path, text) in [
            ("home/notes.txt", "notes"),
            ("home/.ssh/id", "secret"),
            ("home/.cargo/registry/x.rs", "fn x() {}"),
            ("work/src/a.rs", "fn a() {}"),
            ("work/.git/hooks/pre-commit", "#!/bin/sh"),
            ("work/.git/config", "[core]"),
            ("work/.git/HEAD", "ref: refs/heads/main"),
            ("work/sub/.git/hooks/x", "#!/bin/sh"),
            ("data/run/token", "token"),
            ("data/home/admin/workspace/w.txt", "w"),
            ("tmp/t.txt", "t"),
            ("sys/lib.txt", "lib"),
            ("work-other/x.txt", "x"),
        ] {
            file(&root.join(path), text);
        }
        Site { scratch }
    }

    /// 场地里的一处，原样的路径。
    pub fn at(&self, path: &str) -> PathBuf {
        self.scratch.0.join(path)
    }

    /// 场地里的一处，换成真实的位置（得存在）。
    pub fn real(&self, path: &str) -> PathBuf {
        fs::canonicalize(self.at(path)).expect("在")
    }

    /// 照场地造的几个地方：工作区 `work`，数据根 `data`，临时目录 `tmp`，只能读的 `sys`、`home/.cargo`。
    pub fn places(&self) -> Places {
        Places {
            workspace: self.at("work"),
            dirs: Vec::new(),
            data_root: self.at("data"),
            temp: self.at("tmp"),
            readable: vec![self.at("sys"), self.at("home/.cargo")],
        }
    }

    /// 照 [`Site::places`] 造的边界表。
    pub fn boundary(&self) -> Boundary {
        Boundary::new(&self.places())
    }
}
