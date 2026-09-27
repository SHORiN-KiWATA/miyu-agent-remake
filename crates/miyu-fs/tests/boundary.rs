//! 边界表（施工 4-3 上）：几片重叠时照先后，先对上的算。

mod support;

use std::path::PathBuf;

use miyu_fs::{Boundary, Places, Zone};
use support::Site;

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
