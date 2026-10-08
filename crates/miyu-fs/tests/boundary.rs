//! 边界表（施工 4-3 上）：几片重叠时照先后，先对上的算。

use std::path::PathBuf;

use crate::support::Site;
use miyu_fs::{Boundary, Places, Zone, within};

#[test]
fn the_workspace_is_writable_but_what_git_runs_outside_is_read_only() {
    let site = Site::new();
    let boundary = site.boundary();
    assert_eq!(boundary.zone(&site.real("work/src/a.rs")), Zone::Writable);
    assert_eq!(boundary.zone(&site.real("work")), Zone::Writable);
    assert_eq!(boundary.zone(&site.real("work/.git/HEAD")), Zone::Writable);
    assert_eq!(
        boundary.zone(&site.real("work/.git/hooks/pre-commit")),
        Zone::Readable
    );
    assert_eq!(boundary.zone(&site.real("work/.git/hooks")), Zone::Readable);
    assert_eq!(
        boundary.zone(&site.real("work/.git/config")),
        Zone::Readable
    );
    assert_eq!(
        boundary.zone(&site.real("work/sub/.git/hooks/x")),
        Zone::Readable,
        "子仓库里的也算"
    );
    // 还不存在的也照位置算：换成真实的位置由 resolve 做，这里直接接上。
    assert_eq!(
        boundary.zone(&site.real("work").join("new.rs")),
        Zone::Writable
    );
}

#[test]
fn nobody_touches_the_data_root_even_inside_the_temp_dir() {
    let site = Site::new();
    let boundary = site.boundary();
    assert_eq!(boundary.zone(&site.real("data/run/token")), Zone::Forbidden);
    assert_eq!(boundary.zone(&site.real("data")), Zone::Forbidden);
    // 临时目录把数据根包在里面（测试、MIYU_HOME 指到那里）：数据根照样谁都不能碰。
    let wide = Boundary::new(&Places {
        temp: site.scratch.0.clone(),
        ..site.places()
    });
    assert_eq!(wide.zone(&site.real("data/run/token")), Zone::Forbidden);
    assert_eq!(wide.zone(&site.real("tmp/t.txt")), Zone::Writable);
}

#[test]
fn a_workspace_moved_into_the_data_root_is_still_a_workspace() {
    let site = Site::new();
    let boundary = Boundary::new(&Places {
        workspace: site.at("data/home/admin/workspace"),
        ..site.places()
    });
    assert_eq!(
        boundary.zone(&site.real("data/home/admin/workspace/w.txt")),
        Zone::Writable
    );
    assert_eq!(boundary.zone(&site.real("data/run/token")), Zone::Forbidden);
}

/// 加进来的目录（施工 5-10 上）：和工作区一样能读能写。
#[test]
fn an_added_dir_is_writable_like_the_workspace() {
    let site = Site::new();
    let boundary = Boundary::new(&Places {
        dirs: vec![site.at("work-other")],
        ..site.places()
    });
    assert_eq!(
        boundary.zone(&site.real("work-other/x.txt")),
        Zone::Writable
    );
    assert_eq!(boundary.zone(&site.real("work-other")), Zone::Writable);
    assert_eq!(
        boundary.zone(&site.real("home/notes.txt")),
        Zone::Outside,
        "别处照旧"
    );
}

#[test]
fn inside_an_added_dir_what_git_runs_outside_is_read_only() {
    let site = Site::new();
    let boundary = Boundary::new(&Places {
        workspace: site.at("work-other"),
        dirs: vec![site.at("work")],
        ..site.places()
    });
    assert_eq!(boundary.zone(&site.real("work/src/a.rs")), Zone::Writable);
    assert_eq!(
        boundary.zone(&site.real("work/.git/hooks/pre-commit")),
        Zone::Readable
    );
    assert_eq!(
        boundary.zone(&site.real("work/.git/config")),
        Zone::Readable
    );
}

/// 加进来的目录落进了数据根（例如报来以后被换成了链接）：数据根照样谁都不能碰。
#[test]
fn the_data_root_wins_over_an_added_dir() {
    let site = Site::new();
    let boundary = Boundary::new(&Places {
        dirs: vec![site.at("data"), site.at("data/home")],
        ..site.places()
    });
    assert_eq!(boundary.zone(&site.real("data/run/token")), Zone::Forbidden);
    assert_eq!(
        boundary.zone(&site.real("data/home/admin/workspace/w.txt")),
        Zone::Forbidden
    );
}

#[test]
fn an_added_dir_that_does_not_exist_is_left_out() {
    let site = Site::new();
    let boundary = Boundary::new(&Places {
        dirs: vec![site.at("nowhere")],
        ..site.places()
    });
    assert_eq!(
        boundary.zone(&site.real("").join("nowhere/x.txt")),
        Zone::Outside
    );
}

#[test]
fn temp_is_writable_system_and_toolchains_read_only_the_rest_outside() {
    let site = Site::new();
    let boundary = site.boundary();
    assert_eq!(boundary.zone(&site.real("tmp/t.txt")), Zone::Writable);
    assert_eq!(boundary.zone(&site.real("sys/lib.txt")), Zone::Readable);
    assert_eq!(
        boundary.zone(&site.real("home/.cargo/registry/x.rs")),
        Zone::Readable
    );
    assert_eq!(boundary.zone(&site.real("home/.ssh/id")), Zone::Outside);
    assert_eq!(boundary.zone(&site.real("home/notes.txt")), Zone::Outside);
    // 一段一段比：work-other 不在 work 里。
    assert_eq!(boundary.zone(&site.real("work-other/x.txt")), Zone::Outside);
}

#[test]
fn a_place_that_does_not_exist_is_left_out() {
    let site = Site::new();
    let boundary = Boundary::new(&Places {
        readable: vec![site.at("nowhere"), site.at("sys")],
        ..site.places()
    });
    assert_eq!(boundary.zone(&site.real("sys/lib.txt")), Zone::Readable);
    assert_eq!(boundary.zone(&site.real("home/notes.txt")), Zone::Outside);
}

#[test]
fn the_data_root_is_known_in_any_case_where_case_does_not_matter() {
    let site = Site::new();
    let boundary = site.boundary();
    let token = site.real("data/run/token");
    let shouting = PathBuf::from(token.to_string_lossy().to_uppercase());
    let expected = if cfg!(any(target_os = "macos", windows)) {
        Zone::Forbidden
    } else {
        // 分大小写的文件系统上，那是另一个地方。
        Zone::Outside
    };
    assert_eq!(boundary.zone(&shouting), expected, "{}", shouting.display());
}

#[test]
fn within_counts_the_dir_itself_and_folds_case_where_case_does_not_matter() {
    let site = Site::new();
    let data = site.real("data");
    assert!(within(&data, &data), "自己也算");
    assert!(within(&site.real("data/run/token"), &data));
    assert!(!within(&site.real("work"), &data));
    // 只是名字开头一样的兄弟目录不算：照一段一段比，不照字符串比。
    let sibling = PathBuf::from(format!("{}-old", data.display()));
    assert!(!within(&sibling, &data), "{}", sibling.display());
    let shouting = PathBuf::from(data.to_string_lossy().to_uppercase()).join("x");
    assert_eq!(
        within(&shouting, &data),
        cfg!(any(target_os = "macos", windows)),
        "{}",
        shouting.display()
    );
}

#[test]
fn this_machine_has_its_system_and_temp_dirs() {
    let site = Site::new();
    let places = Places::here(site.at("work"), site.at("data"), Some(&site.at("home")));
    // 家目录下的工具链目录都列上了。场地本身在临时目录里，所以不拿它们查是哪一片。
    for name in [".cargo", ".rustup", ".npm", ".gitconfig"] {
        assert!(
            places.readable.contains(&site.at("home").join(name)),
            "{name}"
        );
    }
    let boundary = Boundary::new(&places);
    let temp = std::fs::canonicalize(std::env::temp_dir()).expect("临时目录在");
    assert_eq!(boundary.zone(&temp), Zone::Writable);
    let system = if cfg!(windows) {
        PathBuf::from(std::env::var_os("SystemRoot").expect("Windows 上有 SystemRoot"))
    } else {
        PathBuf::from("/usr")
    };
    let system = std::fs::canonicalize(system).expect("系统目录在");
    assert_eq!(boundary.zone(&system), Zone::Readable);
}
