//! 会话表造子会话（施工 3-8 三补，`docs/blueprint/protocol.md`「会话表」第 7 条）：父会话已经不在表里的（删了、停了）不再
//! 造。删会话时，它或者它的子会话停下之前正在派的，等删完拿到表的锁才轮到造：不拦的话，会留下一个父会话已经进了回收处的
//! 子会话。闲够了退下（施工 V-2 再补，`idle.rs`）：还有人拿着把手的（正在办的请求）不退。

use std::sync::Arc;
use std::time::Duration;

use miyu_kernel::event::{Level, Permission};
use miyu_kernel::id::{AccountId, CommandId, SessionId};
use miyu_session::testkit::Script;
use miyu_session::{Child, Lineage};
use miyu_store::resources::ResourceRoot;
use miyu_tool::Catalog;

use super::{Opening, local};
use crate::Core;
use crate::test_support::{resources, temp_root};

#[tokio::test]
async fn a_child_is_not_created_for_a_parent_gone_from_the_table() {
    let (root, dir) = temp_root("unit");
    let admin = AccountId::parse("alice").expect("合写法");
    let resources = resources();
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
        persona: Some("engineer".to_string()),
        owner: admin.clone(),
        venue: local(),
        permission: Permission {
            level: Level::Workspace,
            read_only: false,
        },
        attended: false,
        cwd: dir.to_string_lossy().into_owned(),
        dirs: Vec::new(),
        model: None,
        preset: None,
    };
    let refused = core.sessions.spawn(&core, child).await;
    assert_eq!(refused, Err("the parent session is gone".to_string()));
    assert!(
        root.sessions(&admin).expect("读得了").is_empty(),
        "什么都没造"
    );
    // 删不掉就留在临时目录里，不影响测试：Windows 上核心开着会话列表的索引（施工 3-8 七补），它的文件删不掉。
    if let Err(error) = std::fs::remove_dir_all(&dir) {
        eprintln!("临时目录留着：{error}");
    }
}

#[tokio::test]
async fn a_session_someone_holds_a_handle_to_stays_in_the_table() {
    let (root, dir) = temp_root("held");
    let admin = AccountId::parse("alice").expect("合写法");
    let core = Arc::new(
        Core::new(
            root,
            ResourceRoot::at(resources()),
            Arc::new(Script::new([])),
            Catalog::default(),
            None,
            admin.clone(),
            "token".to_string(),
        )
        .with_session_idle(Duration::from_millis(50)),
    );
    let opening = Opening {
        owner: admin,
        attended: false,
        oneshot: false,
        model: None,
        venue: None,
        memory: None,
        preset: None,
        group: false,
        chosen: true,
    };
    let cwd = dir.to_string_lossy().into_owned();
    let command = CommandId::parse("c1").expect("合写法");
    let created = core
        .sessions
        .create(&core, command, None, cwd, Vec::new(), opening)
        .await
        .expect("造得出");
    let found = core.sessions.get(&core, &created.id).await.expect("在表里");
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(core.loaded().await, 1, "正在办的请求拿着把手，不退");
    drop(found);
    let gone = tokio::time::timeout(Duration::from_secs(60), async {
        while core.loaded().await != 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(gone.is_ok(), "放下了把手，闲够了退下");
    if let Err(error) = std::fs::remove_dir_all(&dir) {
        eprintln!("临时目录留着：{error}");
    }
}
