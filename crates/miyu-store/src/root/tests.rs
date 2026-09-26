//! 数据根和缓存目录的测试：三个平台的默认位置；XDG；`MIYU_HOME`；空的、相对的；找不到家目录；
//! 在临时目录里建骨架，Unix 上的权限；读进程环境的那一个只找、不建。
//!
//! 测试用的路径都拼在系统的临时目录下面：它在三台机器上都是绝对路径，换了平台，路径的写法
//! 不一样，拼法一样。

use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

/// 家目录：<临时目录>/home/alice。
fn alice() -> PathBuf {
    std::env::temp_dir().join("home").join("alice")
}

/// `LOCALAPPDATA`：<临时目录>/Local。
fn local() -> PathBuf {
    std::env::temp_dir().join("Local")
}

/// 一份快照：在 `platform` 上，家目录是 [`alice`]，`LOCALAPPDATA` 是 [`local`]，别的都没设。
fn env(platform: Platform) -> Env {
    Env {
        platform,
        miyu_home: None,
        home: Some(alice()),
        xdg_data_home: None,
        xdg_cache_home: None,
        local_app_data: Some(local().into_os_string()),
    }
}

fn data_of(env: &Env) -> PathBuf {
    DataRoot::locate(env).unwrap().path().to_path_buf()
}

#[test]
fn each_platform_has_its_default_place() {
    let cases = [
        (
            Platform::Linux,
            alice().join(".local").join("share").join("miyu"),
            alice().join(".cache").join("miyu"),
        ),
        (
            Platform::Macos,
            alice()
                .join("Library")
                .join("Application Support")
                .join("Miyu"),
            alice().join("Library").join("Caches").join("Miyu"),
        ),
        (
            Platform::Windows,
            local().join("Miyu"),
            local().join("Miyu").join("cache"),
        ),
    ];
    for (platform, data, cache) in cases {
        assert_eq!(data_of(&env(platform)), data, "{platform:?}");
        assert_eq!(cache_root(&env(platform)).unwrap(), cache, "{platform:?}");
    }
}

#[test]
fn xdg_is_followed_on_linux_only() {
    let xdg = |platform| Env {
        xdg_data_home: Some(std::env::temp_dir().join("data").into_os_string()),
        xdg_cache_home: Some(std::env::temp_dir().join("cache").into_os_string()),
        ..env(platform)
    };
    assert_eq!(
        data_of(&xdg(Platform::Linux)),
        std::env::temp_dir().join("data").join("miyu")
    );
    assert_eq!(
        cache_root(&xdg(Platform::Linux)).unwrap(),
        std::env::temp_dir().join("cache").join("miyu")
    );
    assert_eq!(
        data_of(&xdg(Platform::Macos)),
        data_of(&env(Platform::Macos))
    );
}

#[test]
fn miyu_home_moves_the_data_root_but_not_the_cache() {
    let elsewhere = std::env::temp_dir().join("elsewhere");
    for platform in [Platform::Linux, Platform::Macos, Platform::Windows] {
        let moved = Env {
            miyu_home: Some(elsewhere.clone().into_os_string()),
            ..env(platform)
        };
        assert_eq!(data_of(&moved), elsewhere, "{platform:?}");
        assert_eq!(
            cache_root(&moved).unwrap(),
            cache_root(&env(platform)).unwrap(),
            "{platform:?}：缓存整台机器共用，不跟着数据根变"
        );
    }
}

#[test]
fn empty_and_relative_settings() {
    let linux = env(Platform::Linux);
    // 空的当没设。
    let empty = Env {
        miyu_home: Some(OsString::new()),
        xdg_data_home: Some(OsString::new()),
        ..linux.clone()
    };
    assert_eq!(data_of(&empty), data_of(&linux));
    // MIYU_HOME 相对的拒绝。
    let relative = Env {
        miyu_home: Some(OsString::from("miyu-data")),
        ..linux.clone()
    };
    assert_eq!(
        DataRoot::locate(&relative),
        Err(RootError::RelativeMiyuHome(PathBuf::from("miyu-data")))
    );
    // XDG 相对的当没设。
    let xdg = Env {
        xdg_data_home: Some(OsString::from("data")),
        xdg_cache_home: Some(OsString::from("cache")),
        ..linux.clone()
    };
    assert_eq!(data_of(&xdg), data_of(&linux));
    assert_eq!(cache_root(&xdg).unwrap(), cache_root(&linux).unwrap());
}

#[test]
fn a_missing_home_is_an_error() {
    for platform in [Platform::Linux, Platform::Macos] {
        let homeless = Env {
            home: None,
            ..env(platform)
        };
        assert_eq!(DataRoot::locate(&homeless), Err(RootError::NoHome));
        assert_eq!(cache_root(&homeless), Err(RootError::NoHome));
        let relative = Env {
            home: Some(PathBuf::from("alice")),
            ..env(platform)
        };
        assert_eq!(DataRoot::locate(&relative), Err(RootError::NoHome));
    }
    for local_app_data in [None, Some(OsString::from("Local"))] {
        let windows = Env {
            local_app_data,
            ..env(Platform::Windows)
        };
        assert_eq!(DataRoot::locate(&windows), Err(RootError::NoLocalAppData));
        assert_eq!(cache_root(&windows), Err(RootError::NoLocalAppData));
    }
}

/// 测试用的临时目录：进程号加序号，用完删掉。
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        Scratch(std::env::temp_dir().join(format!("miyu-store-test-{}-{n}", std::process::id())))
    }

    /// 这个临时目录下的 `data`，当数据根。
    fn root(&self) -> DataRoot {
        DataRoot::locate(&Env {
            miyu_home: Some(self.0.join("data").into_os_string()),
            ..env(Platform::current())
        })
        .unwrap()
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

#[test]
fn the_skeleton_is_built_and_building_it_again_is_fine() {
    let scratch = Scratch::new();
    let root = scratch.root();
    root.prepare().unwrap();
    let dirs = [
        root.path().to_path_buf(),
        root.system(),
        root.homes(),
        root.state(),
        root.run(),
    ];
    for dir in &dirs {
        assert!(dir.is_dir(), "{} 应该建好了", dir.display());
    }
    root.prepare().unwrap();
    // 该是目录的地方是个文件：报错。
    fs::remove_dir(root.run()).unwrap();
    fs::write(root.run(), "not a directory").unwrap();
    assert!(root.prepare().is_err());
}

#[cfg(unix)]
#[test]
fn new_directories_are_only_for_me() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new();
    let root = scratch.root();
    root.prepare().unwrap();
    for dir in [root.path().to_path_buf(), root.system(), root.run()] {
        let mode = fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700, "{}", dir.display());
    }
}

#[cfg(unix)]
#[test]
fn existing_directories_keep_their_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new();
    let root = scratch.root();
    fs::create_dir_all(root.path()).unwrap();
    fs::set_permissions(root.path(), fs::Permissions::from_mode(0o755)).unwrap();
    root.prepare().unwrap();
    let mode = |dir: &Path| fs::metadata(dir).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(root.path()), 0o755, "人自己建的，权限不改");
    assert_eq!(mode(&root.homes()), 0o700, "新建的照样 0700");
}

#[test]
fn the_process_environment_finds_a_root() {
    // 只找，不建：CI 的三台机器上都找得到。
    let env = Env::current();
    DataRoot::locate(&env).unwrap();
    cache_root(&env).unwrap();
}
