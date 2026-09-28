//! 文件：整盘能读，只能写规格里能写的；藏起来的读、列、看元数据、写、执行都不行；工作区在数据根里照样能用；撑着
//! 规格的那几个目录删不掉、改不了名；规格写成经过链接的写法照样管用；中文、带引号的路径照样管用。

use std::fs;
use std::path::Path;

use super::*;
use crate::support::{Dir, serial};

fn at(path: &Path) -> &str {
    path.to_str().expect("路径是 UTF-8")
}

#[test]
fn everything_can_be_read_but_only_writable_places_written() {
    let work = Dir::new();
    let other = Dir::new();
    let notes = real(&other.file("notes.txt", b"shared notes\n"));
    let cwd = real(work.path());
    let spec = spec(&[&cwd], &[]);
    // 整盘能读：规格外的文件、系统的文件、家目录都读得了。
    assert_eq!(
        ok(&run(&spec, &cwd, "/bin/cat", &[at(&notes)])),
        "shared notes\n"
    );
    ok(&run(&spec, &cwd, "/bin/cat", &["/etc/hosts"]));
    let home = std::env::var("HOME").expect("有家目录");
    ok(&run(&spec, &cwd, "/bin/ls", &[&home]));
    // 只能写能写的。
    assert_eq!(ok(&sh(&spec, &cwd, "echo hi > a.txt && cat a.txt")), "hi\n");
    let other_dir = real(other.path());
    for script in [
        format!("echo more >> '{}'", notes.display()),
        format!("touch '{}/new'", other_dir.display()),
        format!("rm '{}'", notes.display()),
        format!("mkdir '{}/d'", other_dir.display()),
    ] {
        denied(&sh(&spec, &cwd, &script));
    }
    assert_eq!(fs::read(&notes).expect("还在"), b"shared notes\n");
    assert!(!other_dir.join("new").exists() && !other_dir.join("d").exists());
    // 空设备、自己的标准输出照常写得了。
    assert_eq!(
        ok(&sh(
            &spec,
            &cwd,
            "echo x > /dev/null && echo out > /dev/stdout"
        )),
        "out\n"
    );
}

#[test]
fn hidden_places_cannot_be_read_listed_looked_at_written_or_run() {
    let _serial = serial();
    let base = Dir::new();
    let root = real(base.path());
    let data = root.join("data");
    fs::create_dir(&data).expect("建得了目录");
    let token = data.join("token");
    fs::write(&token, b"secret\n").expect("写得进");
    let echo = data.join("echo");
    fs::copy("/bin/echo", &echo).expect("拷得了");
    // 数据根落在能写的地方里：藏起来的更深，照样藏着。
    let spec = spec(&[&root], &[&data]);
    denied(&run(&spec, &root, "/bin/cat", &[at(&token)]));
    denied(&run(&spec, &root, "/usr/bin/stat", &[at(&token)]));
    denied(&run(&spec, &root, "/bin/ls", &[at(&data)]));
    denied(&run(&spec, &root, at(&echo), &["ran"]));
    denied(&sh(
        &spec,
        &root,
        &format!("echo x > '{}/new'", data.display()),
    ));
    let exists = format!("test -e '{}' && echo exists || echo no", token.display());
    assert_eq!(ok(&sh(&spec, &root, &exists)), "no\n");
    assert!(!data.join("new").exists());
}

#[test]
fn a_workspace_inside_the_hidden_data_root_is_usable() {
    let base = Dir::new();
    let data = real(base.path()).join("data");
    let workspace = data.join("home/admin/workspace");
    fs::create_dir_all(&workspace).expect("建得了目录");
    let token = data.join("token");
    fs::write(&token, b"secret\n").expect("写得进");
    let spec = spec(&[&workspace], &[&data]);
    assert_eq!(
        ok(&sh(&spec, &workspace, "echo w > a && cat a && pwd -P")),
        format!("w\n{}\n", workspace.display())
    );
    denied(&run(&spec, &workspace, "/bin/cat", &[at(&token)]));
    // 上级都在藏起来的里面：要放行看它们的元数据，不然一层层看上级的程序找不到自己在哪。
    assert_eq!(
        ok(&run(&spec, &workspace, "/bin/realpath", &["a"])),
        format!("{}\n", workspace.join("a").display())
    );
    // cargo 从工作目录往上一层层换成真实的位置，找它的配置：换不成的，它说 `could not canonicalize path`。
    let cargo = std::env::var("CARGO").expect("cargo 跑的测试有 CARGO");
    let out = run(&spec, &workspace, &cargo, &["--version"]);
    ok(&out);
    assert!(out.stderr.is_empty(), "{}", text(&out.stderr));
}

#[test]
fn directories_holding_the_spec_together_cannot_be_renamed_or_removed() {
    let base = Dir::new();
    let root = real(base.path());
    let workspace = root.join("ws");
    let data = root.join("a/data");
    fs::create_dir_all(workspace.join("d")).expect("建得了目录");
    fs::create_dir_all(&data).expect("建得了目录");
    let spec = spec(&[&root, &workspace], &[&data]);
    // 能写的那几片自己、藏起来的在能写的地方里的上级：删了、改了名，下一条命令的规格就对不上了。
    for (from, to) in [
        (workspace.clone(), root.join("ws2")),
        (root.join("a"), root.join("b")),
        (data.clone(), root.join("data2")),
    ] {
        denied(&run(&spec, &root, "/bin/mv", &[at(&from), at(&to)]));
        assert!(from.is_dir(), "{} 还在", from.display());
    }
    // 别的目录照常改名、删。
    ok(&sh(
        &spec,
        &workspace,
        "mv d d2 && mkdir -p e/f && rm -rf d2 e",
    ));
}

#[test]
fn paths_given_through_links_are_still_confined() {
    // 规格写的是经过链接的写法（`/var/folders/…`，真实的位置在 `/private/var/folders/…`）。
    let base = Dir::new();
    let workspace = base.path().join("ws");
    let data = base.path().join("data");
    fs::create_dir_all(&workspace).expect("建得了目录");
    fs::create_dir_all(&data).expect("建得了目录");
    fs::write(data.join("token"), b"secret\n").expect("写得进");
    assert_ne!(real(&workspace), workspace, "临时目录在链接后面");
    let spec = spec(&[&workspace], &[&data]);
    let cwd = real(&workspace);
    assert_eq!(ok(&sh(&spec, &cwd, "echo w > a && cat a")), "w\n");
    denied(&run(
        &spec,
        &cwd,
        "/bin/cat",
        &[at(&real(&data).join("token"))],
    ));
    denied(&run(&spec, &cwd, "/bin/cat", &[at(&data.join("token"))]));
}

#[test]
fn chinese_and_quoted_paths_work() {
    let base = Dir::new();
    let root = real(base.path());
    let chinese = root.join("中文 目录");
    let quoted = root.join("a\"b\\c");
    let hidden = chinese.join("藏起来的");
    fs::create_dir_all(&hidden).expect("建得了目录");
    fs::create_dir_all(&quoted).expect("建得了目录");
    fs::write(hidden.join("f"), b"secret\n").expect("写得进");
    let spec = spec(&[&chinese, &quoted], &[&hidden]);
    assert_eq!(ok(&sh(&spec, &chinese, "echo w > a && cat a")), "w\n");
    assert_eq!(ok(&sh(&spec, &quoted, "echo q > a && cat a")), "q\n");
    denied(&run(&spec, &chinese, "/bin/cat", &[at(&hidden.join("f"))]));
}

#[test]
fn links_and_renames_inside_writable_places_work() {
    let work = Dir::new();
    let cwd = real(work.path());
    let spec = spec(&[&cwd], &[]);
    let script =
        "echo a > h1 && ln h1 h2 && ln -s h1 s && mkdir -p d/e && mv d d2 && rm -rf d2 && cat h2 s";
    assert_eq!(ok(&sh(&spec, &cwd, script)), "a\na\n");
}
