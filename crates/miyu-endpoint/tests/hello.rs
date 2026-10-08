//! 握手回应里的 `host`，`fs.realpath`（施工 W-3，`docs/blueprint/web-module.md`「四、路径」、`protocol.md`
//! 「握手」「`fs.realpath`」）：`host` 三格总有，`workspace` 是真实的位置；握手不替账号造工作区（管理员的工作区
//! 核心起来时就建好了，`core.md`），没建过的就照原样交回。`fs.realpath` 往上找最近在的一层换成真实的位置，
//! 相对的要配 `cwd`，不查边界。

use std::path::Path;

use serde_json::json;

use miyu_session::testkit::Script;

use crate::support::*;

/// 连上、握手，交回客户端。
async fn client(home: &Home) -> Client {
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    client
}

fn touch(root: &Path, relative: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().expect("有上级目录")).expect("建得了目录");
    std::fs::write(&path, "x").expect("写得进");
}

#[tokio::test]
async fn host_has_the_three_fields_when_the_workspace_already_exists() {
    let home = Home::new();
    let system_home = home.work.join("system-home");
    std::fs::create_dir_all(&system_home).expect("建得了目录");
    // 工作区照真核心起来时的样子先建好（`core.md`：管理员的工作区核心起来时就建好了）。
    home.root
        .prepare_home(&alice())
        .expect("建得了家目录、工作区");
    let mut client = Client::connect(home.core_at_home(&Script::new([]), system_home.clone()));
    let reply = client.hello().await;

    let host = &reply["result"]["host"];
    assert_eq!(
        host["home"],
        json!(system_home.display().to_string()),
        "home 照原样：{reply}"
    );
    assert_eq!(
        host["platform"],
        json!(std::env::consts::OS),
        "platform 是这一台机器的：{reply}"
    );

    let workspace = host["workspace"].as_str().expect("workspace 是字符串");
    let expected = std::fs::canonicalize(home.root.workspace(&alice())).expect("在");
    assert_eq!(workspace, expected.display().to_string(), "{reply}");
}

/// 握手不该替每一个连上来的头造目录：没人建过工作区，`host.workspace` 照原样交回、不在磁盘上建出它（施工
/// miyu-cli `edit_leaves_the_file_alone_when_nothing_is_saved` 撞见过：早先握手顺手建了工作区，账号目录下
/// 多出一个不该有的 `workspace`，这个测试守住不再发生）。
#[tokio::test]
async fn host_workspace_is_given_as_is_and_not_created_when_nobody_has_used_the_account_yet() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    let reply = client.hello().await;

    let raw = home.root.workspace(&alice());
    assert_eq!(
        reply["result"]["host"]["workspace"],
        json!(raw.display().to_string()),
        "还没建过，照原样交回：{reply}"
    );
    // 账号目录下可能已经有核心起来时建的会话列表索引（施工 3-8 七补），但不该多出 `workspace`。
    assert!(!raw.exists(), "握手自己不该把工作区建出来：{reply}");
}

#[tokio::test]
async fn host_home_is_null_when_the_system_home_is_not_known() {
    let home = Home::new();
    // 默认的 `home.core` 不设系统的家目录（施工 4-3 下，`core_full` 的 `home` 参数是 `None`）。
    let mut client = Client::connect(home.core(&Script::new([])));
    let hello = client.hello().await;
    let host = hello["result"]
        .get("host")
        .unwrap_or_else(|| panic!("host 这一格总有：{hello}"));
    assert!(host.is_object(), "host 是对象：{hello}");
    assert!(host["home"].is_null(), "没有系统的家目录：{hello}");
}

/// 工作区是账号家目录下固定的一层：先造一份指向别处的符号链接，握手该把它换成链接指的真实位置，不是原样的那个
/// 链接路径（施工 W-3「四、路径」第 1 条：`workspace` 换成真实的位置）。
#[cfg(unix)]
#[tokio::test]
async fn host_workspace_resolves_a_symlink_to_its_real_position() {
    use std::os::unix::fs::symlink;

    let home = Home::new();
    let real = home.work.join("actual-workspace");
    std::fs::create_dir_all(&real).expect("建得了目录");
    std::fs::create_dir_all(home.root.account_dir(&alice())).expect("建得了账号目录");
    symlink(&real, home.root.workspace(&alice())).expect("建得了符号链接");

    let mut client = Client::connect(home.core(&Script::new([])));
    let hello = client.hello().await;
    let workspace = hello["result"]["host"]["workspace"]
        .as_str()
        .expect("workspace 是字符串");
    let expected = std::fs::canonicalize(&real).expect("在");
    assert_eq!(
        workspace,
        expected.display().to_string(),
        "该换成链接指的地方，不是链接本身：{hello}"
    );
}

#[tokio::test]
async fn fs_realpath_expands_tilde_against_home_cwd_is_optional() {
    let home = Home::new();
    let system_home = home.work.join("system-home-2");
    touch(&system_home, "project/file.txt");
    let mut client = Client::connect(home.core_at_home(&Script::new([]), system_home.clone()));
    client.hello().await;

    let reply = client
        .call("r1", "fs.realpath", json!({"path": "~/project"}))
        .await;
    let expected = std::fs::canonicalize(system_home.join("project")).expect("在");
    assert_eq!(
        reply["result"]["path"],
        json!(expected.display().to_string()),
        "{reply}"
    );
}

#[tokio::test]
async fn fs_realpath_resolves_relative_paths_against_cwd_which_may_itself_start_with_tilde() {
    let home = Home::new();
    let system_home = home.work.join("system-home-3");
    touch(&system_home, "sub/file.txt");
    let mut client = Client::connect(home.core_at_home(&Script::new([]), system_home.clone()));
    client.hello().await;

    // `cwd` 本身也是 `~` 开头的写法：先把它换成真实的位置，`path` 再接在它后面。
    let reply = client
        .call(
            "r1",
            "fs.realpath",
            json!({"path": "file.txt", "cwd": "~/sub"}),
        )
        .await;
    let expected = std::fs::canonicalize(system_home.join("sub/file.txt")).expect("在");
    assert_eq!(
        reply["result"]["path"],
        json!(expected.display().to_string()),
        "{reply}"
    );

    // `cwd` 是绝对路径也一样。
    let abs_cwd = std::fs::canonicalize(&system_home).expect("在");
    let reply = client
        .call(
            "r2",
            "fs.realpath",
            json!({"path": "sub/file.txt", "cwd": abs_cwd.display().to_string()}),
        )
        .await;
    assert_eq!(
        reply["result"]["path"],
        json!(expected.display().to_string()),
        "{reply}"
    );
}

#[tokio::test]
async fn fs_realpath_a_relative_path_without_cwd_is_bad_params() {
    let home = Home::new();
    let mut client = client(&home).await;
    let reply = client
        .call("r1", "fs.realpath", json!({"path": "file.txt"}))
        .await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
}

#[tokio::test]
async fn fs_realpath_an_absolute_path_needs_no_cwd() {
    let home = Home::new();
    touch(&home.work, "abs/file.txt");
    let mut client = client(&home).await;
    let target = home.work.join("abs/file.txt").display().to_string();
    let reply = client
        .call("r1", "fs.realpath", json!({"path": target}))
        .await;
    let expected = std::fs::canonicalize(home.work.join("abs/file.txt")).expect("在");
    assert_eq!(
        reply["result"]["path"],
        json!(expected.display().to_string()),
        "{reply}"
    );
}

#[tokio::test]
async fn fs_realpath_climbs_to_the_nearest_existing_ancestor_and_appends_the_rest_as_is() {
    let home = Home::new();
    std::fs::create_dir_all(home.work.join("here")).expect("建得了目录");
    let missing = home.work.join("here/does/not/exist/yet");
    let mut client = client(&home).await;
    let reply = client
        .call(
            "r1",
            "fs.realpath",
            json!({"path": missing.display().to_string()}),
        )
        .await;
    let base = std::fs::canonicalize(home.work.join("here")).expect("在");
    let expected = base.join("does/not/exist/yet");
    assert_eq!(
        reply["result"]["path"],
        json!(expected.display().to_string()),
        "还不存在的那几段原样接在最近在的那一层后面：{reply}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn fs_realpath_resolves_a_symlink_in_the_middle_of_the_path() {
    use std::os::unix::fs::symlink;

    let home = Home::new();
    touch(&home.work, "real/file.txt");
    let link = home.work.join("link");
    symlink(home.work.join("real"), &link).expect("建得了符号链接");

    let mut client = client(&home).await;
    let reply = client
        .call(
            "r1",
            "fs.realpath",
            json!({"path": link.join("file.txt").display().to_string()}),
        )
        .await;
    let expected = std::fs::canonicalize(home.work.join("real/file.txt")).expect("在");
    assert_eq!(
        reply["result"]["path"],
        json!(expected.display().to_string()),
        "链接换成它指的地方，不是链接本身：{reply}"
    );
}

#[tokio::test]
async fn fs_realpath_does_not_check_the_boundary_the_data_root_resolves_too() {
    let home = Home::new();
    let mut client = client(&home).await;
    let reply = client
        .call(
            "r1",
            "fs.realpath",
            json!({"path": home.root.path().display().to_string()}),
        )
        .await;
    assert!(
        reply.get("error").is_none(),
        "fs.realpath 只说位置，不查边界：{reply}"
    );
    let expected = std::fs::canonicalize(home.root.path()).expect("在");
    assert_eq!(
        reply["result"]["path"],
        json!(expected.display().to_string()),
        "{reply}"
    );
}

#[tokio::test]
async fn fs_realpath_without_a_home_a_tilde_path_is_path_unreadable() {
    let home = Home::new();
    // `home.core` 不设系统的家目录：`~` 一层都接不上。
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let reply = client
        .call("r1", "fs.realpath", json!({"path": "~/anything"}))
        .await;
    assert_eq!(reason(&reply), Some("path_unreadable"), "{reply}");
}
