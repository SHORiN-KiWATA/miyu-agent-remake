//! 数据根和缓存目录的测试：三个平台的默认位置；缓存的 XDG；`MIYU_HOME`；空的、相对的；找不到
//! 家目录；在临时目录里建骨架，认标记，Unix 上的权限；读进程环境的那一个只找、不建。
//!
//! 测试用的路径都拼在系统的临时目录下面：它在三台机器上都是绝对路径，换了平台，路径的写法
//! 不一样，拼法一样。

use super::*;
use crate::test_support::Scratch;

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
        xdg_cache_home: None,
        local_app_data: Some(local().into_os_string()),
    }
}

fn data_of(env: &Env) -> PathBuf {
    DataRoot::locate(env).unwrap().path().to_path_buf()
}

#[test]
fn each_platform_has_its_default_place() {
    // 数据根三个平台都是家目录的 .miyu；缓存各有各的地方。
    let cases = [
        (Platform::Linux, alice().join(".cache").join("miyu")),
        (
            Platform::Macos,
            alice().join("Library").join("Caches").join("Miyu"),
        ),
        (Platform::Windows, local().join("Miyu").join("cache")),
    ];
    for (platform, cache) in cases {
        assert_eq!(
            data_of(&env(platform)),
            alice().join(".miyu"),
            "{platform:?}"
        );
        assert_eq!(cache_root(&env(platform)).unwrap(), cache, "{platform:?}");
    }
}

#[test]
fn xdg_moves_the_cache_on_linux_only() {
    let xdg = |platform| Env {
        xdg_cache_home: Some(std::env::temp_dir().join("cache").into_os_string()),
        ..env(platform)
    };
    assert_eq!(
        cache_root(&xdg(Platform::Linux)).unwrap(),
        std::env::temp_dir().join("cache").join("miyu")
    );
    assert_eq!(
        cache_root(&xdg(Platform::Macos)).unwrap(),
        cache_root(&env(Platform::Macos)).unwrap()
    );
    assert_eq!(data_of(&xdg(Platform::Linux)), alice().join(".miyu"));
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
        xdg_cache_home: Some(OsString::new()),
        ..linux.clone()
    };
    assert_eq!(data_of(&empty), data_of(&linux));
    assert_eq!(cache_root(&empty).unwrap(), cache_root(&linux).unwrap());
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
        xdg_cache_home: Some(OsString::from("cache")),
        ..linux.clone()
    };
    assert_eq!(cache_root(&xdg).unwrap(), cache_root(&linux).unwrap());
}

#[test]
fn a_missing_home_is_an_error() {
    for platform in [Platform::Linux, Platform::Macos, Platform::Windows] {
        for home in [None, Some(PathBuf::from("alice"))] {
            let homeless = Env {
                home,
                ..env(platform)
            };
            assert_eq!(
                DataRoot::locate(&homeless),
                Err(RootError::NoHome),
                "{platform:?}"
            );
        }
    }
    let homeless = Env {
        home: None,
        ..env(Platform::Linux)
    };
    assert_eq!(cache_root(&homeless), Err(RootError::NoHome));
    // Windows 的缓存要 LOCALAPPDATA；数据根不要。
    for local_app_data in [None, Some(OsString::from("Local"))] {
        let windows = Env {
            local_app_data,
            ..env(Platform::Windows)
        };
        assert_eq!(data_of(&windows), alice().join(".miyu"));
        assert_eq!(cache_root(&windows), Err(RootError::NoLocalAppData));
    }
}

/// 临时目录下的 `data`，当数据根。
fn root_in(scratch: &Scratch) -> DataRoot {
    DataRoot::locate(&Env {
        miyu_home: Some(scratch.path().join("data").into_os_string()),
        ..env(Platform::current())
    })
    .unwrap()
}

#[test]
fn the_skeleton_is_built_and_building_it_again_is_fine() {
    let scratch = Scratch::new();
    let root = root_in(&scratch);
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
    let root = root_in(&scratch);
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
    let root = root_in(&scratch);
    fs::create_dir_all(root.path()).unwrap();
    fs::set_permissions(root.path(), fs::Permissions::from_mode(0o755)).unwrap();
    root.prepare().unwrap();
    let mode = |dir: &Path| fs::metadata(dir).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(root.path()), 0o755, "人自己建的，权限不改");
    assert_eq!(mode(&root.homes()), 0o700, "新建的照样 0700");
}

/// 这个目录里现在有哪些东西：相对的路径和文件内容，照名字排。
fn contents(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        for entry in fs::read_dir(&next).unwrap() {
            let path = entry.unwrap().path();
            let relative = path.strip_prefix(dir).unwrap().to_path_buf();
            match path.is_dir() {
                true => {
                    found.push((relative, Vec::new()));
                    pending.push(path);
                }
                false => found.push((relative, fs::read(&path).unwrap())),
            }
        }
    }
    found.sort();
    found
}

#[test]
fn a_new_root_gets_the_marker() {
    // 目录不存在、空目录：都写下标记，建好骨架。
    for exists in [false, true] {
        let scratch = Scratch::new();
        let root = root_in(&scratch);
        if exists {
            fs::create_dir_all(root.path()).unwrap();
        }
        root.prepare().unwrap();
        assert_eq!(
            fs::read_to_string(root.path().join(".miyu-root")).unwrap(),
            "This directory is a Miyu data root (layout 1).\n"
        );
        assert!(root.run().is_dir());
        // 有标记的照常用。
        root.prepare().unwrap();
    }
}

#[test]
fn a_directory_that_is_not_ours_is_left_alone() {
    // 只放了一个文件的，和只放了一个目录的：里面是什么不去猜，都只说认不出。
    for dir in [false, true] {
        let scratch = Scratch::new();
        let root = root_in(&scratch);
        fs::create_dir_all(root.path()).unwrap();
        if dir {
            fs::create_dir_all(root.path().join("photos")).unwrap();
            fs::write(root.path().join("photos").join("cat.png"), "猫").unwrap();
        } else {
            fs::write(root.path().join("notes.txt"), "我的笔记").unwrap();
        }
        let before = contents(root.path());
        let Err(error) = root.prepare() else {
            panic!("认不出是 Miyu 的数据根，应该拒绝");
        };
        let PrepareError::NotOurs(path) = &error else {
            panic!("应该是认不出：{error:?}");
        };
        assert_eq!(path, root.path());
        // 报错写明是哪个目录、为什么认不出。
        let said = error.to_string();
        assert!(said.contains(&root.path().display().to_string()), "{said}");
        assert!(said.contains(".miyu-root"), "{said}");
        assert_eq!(contents(root.path()), before, "一个字节都不动");
    }
}

#[test]
fn a_hidden_file_also_makes_it_not_empty() {
    // 宁可多停一回，不往别人的目录里建东西：只有一个隐藏文件，也不算空的。
    let scratch = Scratch::new();
    let root = root_in(&scratch);
    fs::create_dir_all(root.path()).unwrap();
    fs::write(root.path().join(".hidden"), "").unwrap();
    assert!(matches!(root.prepare(), Err(PrepareError::NotOurs(_))));
    assert!(!root.run().exists());
}

#[test]
fn the_process_environment_finds_a_root() {
    // 只找，不建：CI 的三台机器上都找得到。
    let env = Env::current();
    DataRoot::locate(&env).unwrap();
    cache_root(&env).unwrap();
}
