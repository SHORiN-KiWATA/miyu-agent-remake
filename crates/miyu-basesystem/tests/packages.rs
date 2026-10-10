//! `packages`（`docs/blueprint/tools/packages.md`，施工 F-10 上）：列出一个一行，关着的、卸掉的、读不了的另说；看一个照原样交回
//! 核心那一份；看包文件夹装得上的交回看一眼的那一份，写错的说哪里不对、第几行，相对路径、两个都写的不问端口；没有端口说看不了。
//! 给人看的说法中文、英文都换得出字。

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::tool::Access;
use miyu_tool::{Call, Done, Looking, PackageRefusal, PackagesPort};

use crate::support::{Site, check, human, readable, said, tool};

/// 假的端口：列出、看一个、看文件夹各交回给的那一份；数着被问了几次。
struct Port {
    listed: Value,
    info: Result<Value, PackageRefusal>,
    inspected: Result<Value, PackageRefusal>,
    asked: AtomicUsize,
}

impl Port {
    fn new() -> Port {
        Port {
            listed: json!([]),
            info: Ok(json!({})),
            inspected: Ok(json!({})),
            asked: AtomicUsize::new(0),
        }
    }
}

impl PackagesPort for Port {
    fn list(&self) -> Looking<'_> {
        self.asked.fetch_add(1, Ordering::Relaxed);
        let listed = self.listed.clone();
        Box::pin(async move { Ok(listed) })
    }

    fn info(&self, _package: String) -> Looking<'_> {
        self.asked.fetch_add(1, Ordering::Relaxed);
        let info = self.info.clone();
        Box::pin(async move { info })
    }

    fn inspect(&self, _path: PathBuf) -> Looking<'_> {
        self.asked.fetch_add(1, Ordering::Relaxed);
        let inspected = self.inspected.clone();
        Box::pin(async move { inspected })
    }
}

fn refusal(reason: &str, problem: Option<&str>, line: Option<u64>) -> PackageRefusal {
    PackageRefusal {
        reason: reason.to_string(),
        problem: problem.map(str::to_string),
        line,
    }
}

fn text(done: &Done) -> &str {
    match done.blocks.as_slice() {
        [Block::Text(Text { text })] => text,
        other => panic!("只有一段字：{other:?}"),
    }
}

async fn packages(port: &Arc<Port>, args: Value) -> Done {
    let port = Arc::clone(port) as Arc<dyn PackagesPort>;
    Site::new()
        .done_with_packages("packages", args, Some(port))
        .await
}

/// 一个绝对路径：Windows 上 `/x` 不算绝对的。
fn absolute(name: &str) -> String {
    std::env::temp_dir()
        .join(name)
        .to_string_lossy()
        .into_owned()
}

/// 参数是 `args` 的一次调用，别的都空着：只拿来问要碰的路径。
fn bare(args: Value) -> Call {
    Call {
        args: args.to_string(),
        cwd: String::new(),
        home: None,
        data_root: None,
        seen: Default::default(),
        stop: Default::default(),
        sandbox: None,
        log: None,
        offset: miyu_kernel::time::UtcOffset::UTC,
        agents: None,
        messages: None,
        jobs: None,
        sessions: None,
        usage: None,
        questions: None,
        memory: None,
        packages: None,
        ids: None,
    }
}

#[test]
fn it_only_reads_and_names_the_folder_it_looks_at() {
    let tool = tool("packages");
    assert_eq!(tool.spec().access, Access::Read);
    let targets = tool.targets(&bare(json!({"path": "/w/pane"})));
    assert_eq!(targets.len(), 1, "看文件夹的照读那个路径判");
    assert_eq!(targets[0].path, "/w/pane");
    assert!(!targets[0].write && !targets[0].itself);
    assert!(tool.targets(&bare(json!({}))).is_empty(), "列出不碰路径");
    assert!(tool.targets(&bare(json!({"package": "x"}))).is_empty());
}

#[tokio::test]
async fn the_list_is_one_line_each() {
    let mut checked = Vec::new();
    let port = Arc::new(Port {
        listed: json!([
            {"package": "basesystem", "kind": "builtin", "name": "Base system", "summary": "Files and commands"},
            {"package": "echo", "kind": "process", "name": "Echo", "summary": "Says it back", "enabled": false},
            {"package": "pane", "kind": "ui", "name": "Pane"},
            {"package": "net", "kind": "builtin", "name": "Net", "summary": "Web search", "removed": true},
            {"package": "bad", "layer": "home", "code": "bad_toml", "problem": "line 1 is not TOML"},
        ]),
        ..Port::new()
    });
    let done = packages(&port, json!({})).await;
    assert!(!done.error);
    assert_eq!(
        text(&done),
        "basesystem: Base system. Files and commands\n\
         echo: Echo. Says it back Turned off.\n\
         pane: Pane.\n\
         net: Net. Removed.\n\
         bad: cannot be read: line 1 is not TOML\n"
    );
    check(&mut checked, human(done), said("packages/listed"));
    readable(&checked, &["packages"]);
}

#[tokio::test]
async fn one_package_is_the_core_answer_as_it_is() {
    let mut checked = Vec::new();
    let info = json!({"package": "echo", "name": "Echo", "files": 3, "size": 120});
    let port = Arc::new(Port {
        info: Ok(info.clone()),
        ..Port::new()
    });
    let done = packages(&port, json!({"package": "echo"})).await;
    assert_eq!(text(&done), format!("{info}\n"));
    check(
        &mut checked,
        human(done),
        said("packages/shown").with("package", "echo"),
    );
    let port = Arc::new(Port {
        info: Err(refusal("unknown_package", None, None)),
        ..Port::new()
    });
    let done = packages(&port, json!({"package": "nope"})).await;
    assert!(done.error);
    assert_eq!(text(&done), "No package named nope.\n");
    check(&mut checked, human(done), said("packages/failed"));
    readable(&checked, &[]);
}

#[tokio::test]
async fn a_folder_is_checked_before_it_is_installed() {
    let mut checked = Vec::new();
    let path = absolute("pane");
    let preview = json!({"package": "pane", "program": "ui", "files": 1, "size": 40});
    let port = Arc::new(Port {
        inspected: Ok(preview.clone()),
        ..Port::new()
    });
    let done = packages(&port, json!({"path": path})).await;
    assert_eq!(
        text(&done),
        format!("{path} is a valid package. Installing it gives:\n{preview}\n")
    );
    check(
        &mut checked,
        human(done),
        said("packages/inspected").with("path", path.as_str()),
    );
    let port = Arc::new(Port {
        inspected: Err(refusal(
            "package_invalid",
            Some("Unknown key package.page"),
            Some(5),
        )),
        ..Port::new()
    });
    let done = packages(&port, json!({"path": path})).await;
    assert!(done.error);
    assert_eq!(
        text(&done),
        format!("{path} is not a valid package, line 5: Unknown key package.page\n")
    );
    let port = Arc::new(Port {
        inspected: Err(refusal("path_unreadable", None, None)),
        ..Port::new()
    });
    let done = packages(&port, json!({"path": path})).await;
    assert_eq!(
        text(&done),
        format!("Cannot look at {path}: path_unreadable.\n")
    );
    readable(&checked, &[]);
}

#[tokio::test]
async fn wrong_arguments_never_reach_the_core() {
    let port = Arc::new(Port::new());
    let relative = packages(&port, json!({"path": "pane"})).await;
    assert_eq!(text(&relative), "path must be absolute.\n");
    let both = packages(&port, json!({"package": "a", "path": absolute("a")})).await;
    assert_eq!(text(&both), "Give package or path, not both.\n");
    assert_eq!(port.asked.load(Ordering::Relaxed), 0);
    let none = Site::new()
        .done_with_packages("packages", json!({}), None)
        .await;
    assert!(none.error);
    assert_eq!(text(&none), "Packages cannot be looked at here.\n");
}
