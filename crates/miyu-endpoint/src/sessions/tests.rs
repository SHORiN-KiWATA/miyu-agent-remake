//! 会话表造子会话（施工 3-8 三补，`docs/blueprint/protocol.md`「会话表」第 7 条）：父会话已经不在表里的（删了、停了）不再
//! 造。删会话时，它或者它的子会话停下之前正在派的，等删完拿到表的锁才轮到造：不拦的话，会留下一个父会话已经进了回收处的
//! 子会话。

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use miyu_kernel::event::{Level, Permission};
use miyu_kernel::id::{AccountId, CommandId, SessionId};
use miyu_session::testkit::Script;
use miyu_session::{Child, Lineage};
use miyu_store::env::{Env, Platform};
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;
use miyu_tool::Catalog;

use super::local;
use crate::Core;

#[tokio::test]
async fn a_child_is_not_created_for_a_parent_gone_from_the_table() {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("miyu-endpoint-unit-{}-{n}", std::process::id()));
    let root = DataRoot::locate(&Env {
        platform: Platform::current(),
        miyu_home: Some(dir.clone().into_os_string()),
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    })
    .expect("MIYU_HOME 是绝对路径");
    root.prepare().expect("建得了骨架");
    let admin = AccountId::parse("alice").expect("合写法");
    let resources = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let core = Arc::new(Core::new(
        root.clone(),
        ResourceRoot::at(resources),
        Arc::new(Script::new([])),
        Catalog::default(),
        None,
        admin.clone(),
        "token".to_string(),
    ));
    let parent = SessionId::parse("01900000-0000-7000-8000-000000000001").expect("合写法");
    let child = Child {
        command: CommandId::parse(&format!("{parent}/j1")).expect("合写法"),
        lineage: Lineage { parent, depth: 1 },
        persona: "engineer".to_string(),
        owner: admin.clone(),
        venue: local(),
        permission: Permission {
            level: Level::Workspace,
            read_only: false,
        },
        attended: false,
        cwd: dir.to_string_lossy().into_owned(),
        dirs: Vec::new(),
    };
    let refused = core.sessions.spawn(&core, child).await;
    assert_eq!(refused, Err("the parent session is gone".to_string()));
    assert!(
        root.sessions(&admin).expect("读得了").is_empty(),
        "什么都没造"
    );
    std::fs::remove_dir_all(&dir).expect("删得掉");
}
