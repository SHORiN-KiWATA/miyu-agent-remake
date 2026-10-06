//! 真核心上 `miyu config` 认人敲的键（施工 8-3 补，`docs/blueprint/cli/config.md`「怎么走」第 8 条）：带多余引号的模型键
//! 照核心的写法规整，`set`、`explain`、`unset` 找得到回应；模型那一块是 `next_turn`，说「下一轮生效」。

mod support;

use miyu_ipc::connect_or_start;
use support::cli::{run, stderr, stdout};
use support::{Home, within};

/// 施工 8-3 补：人敲带多余引号的键（模型名里没有点），`set`、`explain`、`unset` 照规整过的键找得到核心的回应、印出来；
/// 模型那一块是 `next_turn`，说「下一轮生效」，不说「当场生效」。
#[tokio::test]
async fn a_quoted_model_key_is_found_and_next_turn_is_said() {
    let home = Home::new();
    let root = home.root.path().to_path_buf();
    let (held, _) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let cwd = std::env::temp_dir();
    let typed = "providers.dev.models.\"m-1\".window";
    let key = "providers.dev.models.m-1.window";
    let set = run(
        &root,
        &cwd,
        "zh_CN.UTF-8",
        &["config", "set", typed, "4096"],
    )
    .await;
    assert_eq!(
        (set.status.code(), stderr(&set)),
        (
            Some(0),
            format!("· {key} = 4096 写进了个人设置，下一轮生效\n")
        )
    );
    let explain = run(&root, &cwd, "zh_CN.UTF-8", &["config", "explain", typed]).await;
    assert_eq!(explain.status.code(), Some(0), "{explain:?}");
    let shown = stdout(&explain);
    let first = shown.lines().next().unwrap_or_default();
    assert!(
        first.contains(&format!("（{key}）")) && first.ends_with("下一轮生效。"),
        "{shown}"
    );
    assert!(
        shown
            .lines()
            .any(|line| line.trim_start().starts_with("4096") && line.contains("个人设置")),
        "{shown}"
    );
    let unset = run(&root, &cwd, "zh_CN.UTF-8", &["config", "unset", typed]).await;
    assert_eq!(unset.status.code(), Some(0), "{unset:?}");
    assert!(
        stderr(&unset).starts_with(&format!("· 从个人设置里删掉了 {key}，现在是 ")),
        "{unset:?}"
    );
    let again = run(&root, &cwd, "zh_CN.UTF-8", &["config", "unset", typed]).await;
    assert_eq!(
        (again.status.code(), stderr(&again)),
        (Some(0), format!("· 个人设置里本来就没写 {key}\n"))
    );
    drop(held);
}
