//! 外部身份的会话的沙盒（施工 5-12 下，`11-权限与沙盒.md` 第三节「外部身份」）：关得住读的平台上，命令照名单只读系统目录、
//! `/dev`、`/proc/self` 和工作区，只写工作区和工作区里的 `.tmp`（`TMPDIR` 指到它），不看级别（完全放开也关进来）；关不住读的
//! 平台上权限策略拒命令，假工具一次都没跑。

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use miyu_kernel::event::{Level, Permission};
use miyu_kernel::id::VenueId;
use miyu_kernel::tool::Access;
use miyu_sandbox::Sandboxed;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

/// 假的助手：假工具不起它。
const HELPER: &str = "miyu-sandbox";

/// 群会话（属主不是管理员），工作区 `work/`，沙盒能用，级别 `level`：执行命令的假工具 `run` 调一次，交回它拿到的沙盒和
/// 跑了几次。
async fn confined(level: Level) -> (Home, PathBuf, Vec<Option<Arc<Sandboxed>>>) {
    let home = Home::outside_temp();
    let work = home.scratch.0.join("work");
    std::fs::create_dir_all(&work).expect("建得了工作区");
    let run = Fake::new("run", Access::Execute, Act::Echo);
    let tools = Catalog::new([Arc::clone(&run) as Arc<dyn Tool>]).expect("合写法");
    let script = Script::new([Play::calls(&[("run", "{}")]), Play::Says("好。")]);
    let opening = Opening {
        permission: Permission {
            level,
            read_only: false,
        },
        attended: false,
        cwd: work.to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: Some(PathBuf::from(HELPER)),
        sandbox_cache: None,
    };
    let lines = Lines {
        venue: VenueId::parse("qq:group:1").expect("场所合写法"),
        owner_is_admin: false,
        ..Lines::default()
    };
    let handle = home.create_full(&script, &tools, opening, lines).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let sandboxes = run.calls().into_iter().map(|call| call.sandbox).collect();
    let work = std::fs::canonicalize(&work).expect("在");
    (home, work, sandboxes)
}

#[tokio::test]
async fn a_confined_command_reads_and_writes_only_its_workspace() {
    for level in [Level::Workspace, Level::Full] {
        let (home, work, sandboxes) = confined(level.clone()).await;
        if !miyu_sandbox::CONFINES_READS {
            assert!(sandboxes.is_empty(), "关不住读的平台上不跑");
            continue;
        }
        let [Some(sandboxed)] = sandboxes.as_slice() else {
            panic!("{level:?}：跑一次、带着沙盒：{sandboxes:?}");
        };
        let temp = work.join(".tmp");
        assert!(temp.is_dir(), "工作区里的临时目录建好了");
        assert_eq!(
            sandboxed.spec.write,
            [work.clone(), temp.clone()],
            "{level:?}"
        );
        let read = sandboxed.spec.read.clone().expect("只准读名单里的");
        for wanted in [Path::new("/dev"), Path::new("/proc/self"), &work, &temp] {
            assert!(
                read.iter().any(|path| path == wanted),
                "{wanted:?} 在名单里：{read:?}"
            );
        }
        assert!(
            !read
                .iter()
                .any(|path| path == Path::new("/proc") || path == Path::new("/")),
            "整个 /proc、根目录不在名单里：{read:?}"
        );
        if let Ok(user_home) = std::env::var("HOME") {
            assert!(
                !read.iter().any(|path| path == Path::new(&user_home)),
                "家目录不在名单里"
            );
        }
        assert_eq!(
            sandboxed.env,
            [(OsString::from("TMPDIR"), temp.clone().into_os_string())]
        );
        assert_eq!(
            sandboxed.spec.hidden,
            [std::fs::canonicalize(home.root.path()).expect("在")]
        );
    }
}
