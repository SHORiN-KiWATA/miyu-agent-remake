//! 一次调用带的沙盒（`docs/blueprint/session/tools.md` 第 1a 条，施工 5-4 上）：执行器照派出去那一刻实际生效的那一级
//! 写规格；数据根落在临时目录里的，沙盒用自己的临时目录；工作区这一级工具链的缓存用沙盒自己的一份（施工 5-4 下）；
//! 写不成的不跑。Unix 上有收紧手段的，真的经助手跑 `shell`：
//! 工作区里写得进，外面写不进，外面读得到，数据根读不到；只读时哪儿都写不进。

mod support;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[cfg(unix)]
use miyu_kernel::event::{Body, ToolResult, ToolStatus};
use miyu_kernel::event::{Level, Permission};
use miyu_kernel::tool::Access;
use miyu_sandbox::{Sandboxed, Spec};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Handle, SandboxCache};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use support::*;

/// 假的助手：假工具不起它。
const HELPER: &str = "miyu-sandbox";

/// 真实的位置。
fn real(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).expect("在")
}

/// 在 `cwd` 里造一个会话，沙盒能用，没人能确认：执行命令的假工具 `run` 跑一次，交回它拿到的沙盒。
async fn attached(home: &Home, cwd: &str, permission: Permission) -> Option<Arc<Sandboxed>> {
    cached(home, cwd, permission, None).await
}

/// 同 [`attached`]，沙盒的缓存是 `cache`（施工 5-4 下）。
async fn cached(
    home: &Home,
    cwd: &str,
    permission: Permission,
    cache: Option<SandboxCache>,
) -> Option<Arc<Sandboxed>> {
    added(home, cwd, permission, cache, Vec::new()).await
}

/// 同 [`cached`]，加进来的目录是 `dirs`（施工 5-10 上）。
async fn added(
    home: &Home,
    cwd: &str,
    permission: Permission,
    cache: Option<SandboxCache>,
    dirs: Vec<String>,
) -> Option<Arc<Sandboxed>> {
    let run = Fake::new("run", Access::Execute, Act::Echo);
    let tools = Catalog::new([Arc::clone(&run) as Arc<dyn Tool>]).expect("合写法");
    let script = Script::new([Play::calls(&[("run", "{}")]), Play::Says("好。")]);
    let opening = Opening {
        permission,
        attended: false,
        cwd: cwd.to_string(),
        dirs,
        sandbox: Some(PathBuf::from(HELPER)),
        sandbox_cache: cache,
    };
    let handle = home.create_as(&script, &tools, opening).await;
    turn(&handle).await;
    let calls = run.calls();
    assert_eq!(calls.len(), 1, "跑了一次");
    calls[0].sandbox.clone()
}

/// 说一句，等这一轮说完。
async fn turn(handle: &Handle) {
    let mut pushes = watch(handle).await;
    ask(handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

fn permission(level: Level, read_only: bool) -> Permission {
    Permission { level, read_only }
}

/// 日志里的工具结果，照先后。
#[cfg(unix)]
fn results(home: &Home, handle: &Handle) -> Vec<ToolResult> {
    home.log(handle.id())
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn each_level_gets_its_own_spec() {
    let home = Home::outside_temp();
    let work = home.scratch.0.join("work");
    let data = real(home.root.path());
    let cwd = work.to_string_lossy().into_owned();
    let workspace = attached(&home, &cwd, permission(Level::Workspace, false)).await;
    assert_eq!(
        workspace.as_deref(),
        Some(&Sandboxed {
            helper: PathBuf::from(HELPER),
            spec: Spec {
                write: vec![real(&work), real(&std::env::temp_dir())],
                hidden: vec![data.clone()],
            },
            env: Vec::new(),
        }),
        "工作区：能写工作目录、临时目录，藏数据根"
    );
    let read_only = Sandboxed {
        helper: PathBuf::from(HELPER),
        spec: Spec {
            write: Vec::new(),
            hidden: vec![data],
        },
        env: Vec::new(),
    };
    for level in [
        permission(Level::Workspace, true),
        permission(Level::Full, true),
        permission(Level::Other("root".to_string()), false),
    ] {
        assert_eq!(
            attached(&home, &cwd, level.clone()).await.as_deref(),
            Some(&read_only),
            "{level:?}：只读，哪儿都不能写；不认识的级别按只读"
        );
    }
    assert_eq!(
        attached(&home, &cwd, permission(Level::Full, false)).await,
        None,
        "完全放开：不进沙盒"
    );
    // 工作目录照权限策略的办法换成真实的位置：`~` 接家目录。
    std::fs::create_dir_all(home.home.join("proj")).expect("建得了目录");
    let tilde = attached(&home, "~/proj", permission(Level::Workspace, false))
        .await
        .expect("工作区进沙盒");
    assert_eq!(tilde.spec.write[0], real(&home.home.join("proj")));
}

/// 加进来的目录（施工 5-10 上）：工作区这一级能写，排在工作目录后面，`~` 接家目录；只读照旧哪儿都写不了。
#[tokio::test]
async fn added_dirs_are_writable_at_the_workspace_level() {
    let home = Home::outside_temp();
    let work = home.scratch.0.join("work");
    let extra = home.scratch.0.join("extra");
    for dir in [&work, &extra, &home.home.join("proj")] {
        std::fs::create_dir_all(dir).expect("建得了目录");
    }
    let cwd = work.to_string_lossy().into_owned();
    let dirs = vec![extra.to_string_lossy().into_owned(), "~/proj".to_string()];
    let workspace = added(
        &home,
        &cwd,
        permission(Level::Workspace, false),
        None,
        dirs.clone(),
    )
    .await
    .expect("工作区进沙盒");
    assert_eq!(
        workspace.spec.write,
        vec![
            real(&work),
            real(&extra),
            real(&home.home.join("proj")),
            real(&std::env::temp_dir())
        ]
    );
    let read_only = added(&home, &cwd, permission(Level::Workspace, true), None, dirs)
        .await
        .expect("只读也进沙盒");
    assert!(read_only.spec.write.is_empty(), "只读哪儿都写不了");
}

#[tokio::test]
async fn a_data_root_in_temp_gets_a_temp_dir_of_its_own() {
    let home = Home::new();
    let work = home.scratch.0.join("work");
    std::fs::create_dir_all(&work).expect("建得了目录");
    let data = real(home.root.path());
    let mut name = data.file_name().expect("有名字").to_os_string();
    name.push("-sandbox-tmp");
    let own = data.with_file_name(name);
    // 两次：没有的建上，有了的照用。
    for _ in 0..2 {
        let sandboxed = attached(
            &home,
            &work.to_string_lossy(),
            permission(Level::Workspace, false),
        )
        .await
        .expect("工作区进沙盒");
        assert_eq!(sandboxed.spec.write, vec![real(&work), own.clone()]);
        assert_eq!(sandboxed.spec.hidden, vec![data.clone()]);
        assert_eq!(
            sandboxed.env,
            vec![(OsString::from("TMPDIR"), own.clone().into_os_string())]
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::symlink_metadata(&own)
            .expect("建了")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700, "只给本人");
    }
}

/// 沙盒自己的临时目录别人进得去：不用它，这次调用不跑，照崩了交回。
#[cfg(unix)]
#[tokio::test]
async fn a_temp_dir_others_can_enter_is_not_used() {
    use std::os::unix::fs::PermissionsExt;

    let home = Home::new();
    let data = real(home.root.path());
    let mut name = data.file_name().expect("有名字").to_os_string();
    name.push("-sandbox-tmp");
    let own = data.with_file_name(name);
    std::fs::create_dir(&own).expect("建得了目录");
    std::fs::set_permissions(&own, std::fs::Permissions::from_mode(0o755)).expect("改得了");
    let run = Fake::new("run", Access::Execute, Act::Echo);
    let tools = Catalog::new([Arc::clone(&run) as Arc<dyn Tool>]).expect("合写法");
    let script = Script::new([Play::calls(&[("run", "{}")]), Play::Says("好。")]);
    let opening = Opening {
        permission: permission(Level::Workspace, false),
        attended: false,
        cwd: home.scratch.0.to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: Some(PathBuf::from(HELPER)),
        sandbox_cache: None,
    };
    let handle = home.create_as(&script, &tools, opening).await;
    turn(&handle).await;
    assert!(run.calls().is_empty(), "没跑");
    let results = results(&home, &handle);
    assert_eq!(results.len(), 1, "{results:?}");
    assert_eq!(results[0].status, ToolStatus::Error);
    assert_eq!(
        results[0].blocks,
        [miyu_kernel::block::Block::Text(miyu_kernel::block::Text {
            text: "The tool \"run\" stopped because of an internal error. It may have been partly done.\n"
                .into(),
        })]
    );
}

/// 真的经助手跑 `shell`（Unix 上有收紧手段的）：工作区里写得进，外面写不进、读得到，数据根读不到；只读时工作区里也
/// 写不进。
#[cfg(unix)]
#[tokio::test]
async fn commands_really_run_in_the_sandbox() {
    let helper = miyu_sandbox::testkit::built_helper();
    if !can_confine(&helper) {
        return;
    }
    let home = Home::outside_temp();
    let work = home.scratch.0.join("work");
    let other = home.scratch.0.join("other");
    std::fs::write(other.join("readable.txt"), "readable\n").expect("写得进");
    std::fs::write(home.root.path().join("marker"), "secret\n").expect("写得进");
    let outside = other.join("outside.txt");
    let marker = home.root.path().join("marker");
    let command = format!(
        "echo in > inside.txt && echo wrote-inside; \
         (echo out > '{}') 2>/dev/null && echo wrote-outside || echo blocked-outside; \
         cat '{}'; \
         cat '{}' 2>/dev/null || echo blocked-data",
        outside.display(),
        other.join("readable.txt").display(),
        marker.display()
    );
    let output = shell(&home, &helper, &work, false, &command).await;
    assert_eq!(
        output,
        "wrote-inside\nblocked-outside\nreadable\nblocked-data\n"
    );
    assert!(work.join("inside.txt").exists());
    assert!(!outside.exists());
    let output = shell(
        &home,
        &helper,
        &work,
        true,
        "(echo in > again.txt) 2>/dev/null && echo wrote || echo blocked",
    )
    .await;
    assert_eq!(output, "blocked\n", "只读：哪儿都写不进");
    assert!(!work.join("again.txt").exists());
}

/// 这台机器上助手有没有收紧的手段：没有的（例如没有 Landlock 的内核），这个测试到此为止。
#[cfg(unix)]
fn can_confine(helper: &Path) -> bool {
    let probe = miyu_sandbox::probe(helper, std::time::Duration::from_secs(5)).expect("探得了");
    !probe.mechanisms.is_empty()
}

/// 在 `work` 里，工作区这一级（`read_only` 时只读），经助手 `helper` 跑一条 `shell`，交回给她看的输出。
#[cfg(unix)]
async fn shell(home: &Home, helper: &Path, work: &Path, read_only: bool, command: &str) -> String {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let tools = Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来"))
        .expect("合写法");
    let args = serde_json::json!({ "command": command, "description": "Test" }).to_string();
    let script = Script::new([Play::calls(&[("shell", args.as_str())]), Play::Says("好。")]);
    let opening = Opening {
        permission: permission(Level::Workspace, read_only),
        attended: false,
        cwd: work.to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: Some(helper.to_path_buf()),
        sandbox_cache: None,
    };
    let handle = home.create_as(&script, &tools, opening).await;
    turn(&handle).await;
    let results = results(home, &handle);
    assert_eq!(results.len(), 1, "{results:?}");
    assert_eq!(results[0].status, ToolStatus::Ok, "{results:?}");
    results[0]
        .blocks
        .iter()
        .map(|block| match block {
            miyu_kernel::block::Block::Text(text) => text.text.clone(),
            other => panic!("只该有字：{other:?}"),
        })
        .collect()
}

/// 场地里的沙盒的缓存：`cache/sandbox/alice`，你的 cargo 目录是假家目录下的 `.cargo`。
fn toolchain_cache(home: &Home) -> SandboxCache {
    SandboxCache {
        dir: home.scratch.0.join("cache").join("sandbox").join("alice"),
        cargo_home: Some(home.home.join(".cargo")),
    }
}

#[tokio::test]
async fn the_workspace_level_gets_its_own_toolchain_caches() {
    let home = Home::outside_temp();
    let work = home.scratch.0.join("work");
    let cargo = home.home.join(".cargo");
    std::fs::create_dir_all(&cargo).expect("建得了目录");
    std::fs::write(cargo.join("config.toml"), "[net]\nretry = 3\n").expect("写得进");
    std::fs::write(cargo.join("credentials.toml"), "token = \"secret\"\n").expect("写得进");
    let cache = toolchain_cache(&home);
    let cwd = work.to_string_lossy().into_owned();
    let sandboxed = cached(
        &home,
        &cwd,
        permission(Level::Workspace, false),
        Some(cache.clone()),
    )
    .await
    .expect("工作区进沙盒");
    let dir = real(&cache.dir);
    assert_eq!(
        sandboxed.spec.write,
        vec![real(&work), real(&std::env::temp_dir()), dir.clone()]
    );
    let expected: Vec<(OsString, OsString)> = [
        ("CARGO_HOME", dir.join("cargo")),
        ("npm_config_cache", dir.join("npm")),
        ("PIP_CACHE_DIR", dir.join("pip")),
        ("GOMODCACHE", dir.join("go").join("mod")),
        ("GOCACHE", dir.join("go").join("build")),
    ]
    .into_iter()
    .map(|(name, place)| (OsString::from(name), place.into_os_string()))
    .collect();
    assert_eq!(sandboxed.env, expected);
    // cargo 的配置带过去了，发布用的令牌没带。
    let carried = dir.join("cargo").join("config.toml");
    assert_eq!(
        std::fs::read_to_string(&carried).expect("带过去了"),
        "[net]\nretry = 3\n"
    );
    #[cfg(unix)]
    assert_eq!(
        std::fs::read_link(&carried).expect("是链接"),
        cargo.join("config.toml")
    );
    assert!(
        !dir.join("cargo").join("credentials.toml").exists(),
        "令牌不带"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&dir).expect("建了").permissions().mode();
        assert_eq!(mode & 0o777, 0o700, "只给本人");
    }
    // 只读、完全放开：不放、不设。
    let read_only = cached(
        &home,
        &cwd,
        permission(Level::Workspace, true),
        Some(cache.clone()),
    )
    .await
    .expect("只读进沙盒");
    assert!(
        read_only.spec.write.is_empty() && read_only.env.is_empty(),
        "{read_only:?}"
    );
    assert_eq!(
        cached(&home, &cwd, permission(Level::Full, false), Some(cache)).await,
        None
    );
}

/// cargo 的配置照你的来：指错了的换掉，你的没了就把带过去的删掉（施工 5-4 下）。
#[cfg(unix)]
#[tokio::test]
async fn the_carried_cargo_config_follows_yours() {
    let home = Home::outside_temp();
    let cwd = home.scratch.0.join("work").to_string_lossy().into_owned();
    let cargo = home.home.join(".cargo");
    std::fs::create_dir_all(&cargo).expect("建得了目录");
    std::fs::write(cargo.join("config.toml"), "[net]\n").expect("写得进");
    let cache = toolchain_cache(&home);
    let carried = cache.dir.join("cargo").join("config.toml");
    let workspace = || permission(Level::Workspace, false);
    // 第一次：建好只给本人的缓存和链接。再换成一条指错了的。
    cached(&home, &cwd, workspace(), Some(cache.clone())).await;
    std::fs::remove_file(&carried).expect("删得了");
    let wrong = home.scratch.0.join("other").join("wrong.toml");
    std::fs::write(&wrong, "").expect("写得进");
    std::os::unix::fs::symlink(&wrong, &carried).expect("造得了链接");
    cached(&home, &cwd, workspace(), Some(cache.clone())).await;
    assert_eq!(
        std::fs::read_link(&carried).expect("是链接"),
        cargo.join("config.toml"),
        "指错了的换掉"
    );
    std::fs::remove_file(cargo.join("config.toml")).expect("删得了");
    cached(&home, &cwd, workspace(), Some(cache)).await;
    assert!(
        std::fs::symlink_metadata(&carried).is_err(),
        "你的没了，带过去的删掉"
    );
}
