//! 排队的任务的号（施工 9-5）：先拿收推送的一头，再报的变化排在要列表前面，列表记的号盖住它们；之后的号比它大。转发任务照它
//! 丢掉号不大于它的，所以回应以后推来的都比回应里的列表新。

use std::sync::Arc;

use miyu_kernel::id::{AccountId, SessionId};
use miyu_session::testkit::Script;
use miyu_store::resources::ResourceRoot;
use miyu_tool::Catalog;

use crate::Core;
use crate::test_support::{resources, temp_root};

fn core() -> Arc<Core> {
    let (root, _) = temp_root("listing");
    let resources = resources();
    Arc::new(Core::new(
        root,
        ResourceRoot::at(&resources),
        Arc::new(Script::new([])),
        Catalog::default(),
        None,
        AccountId::parse("admin").expect("合写法"),
        "token".to_string(),
    ))
}

fn session(n: u8) -> SessionId {
    SessionId::parse(&format!("0192f3a0-1111-7abc-8def-0011223344{n:02}")).expect("合写法")
}

#[tokio::test]
async fn the_list_covers_what_came_before_it() {
    let core = core();
    let mut pushes = core.listing.subscribe();
    core.listing.removed(&core, session(1));
    core.listing.removed(&core, session(2));
    let snapshot = core.listing.snapshot(&core).await.expect("列得出来");
    assert!(snapshot.sessions.is_empty());
    core.listing.removed(&core, session(3));
    let numbers: Vec<u64> = [
        pushes.recv().await.expect("推了"),
        pushes.recv().await.expect("推了"),
        pushes.recv().await.expect("推了"),
    ]
    .iter()
    .map(|change| change.number)
    .collect();
    assert_eq!(numbers, [1, 2, 3]);
    assert_eq!(snapshot.number, 2, "列表前面的两条它都盖住了，后面那条没有");
}
