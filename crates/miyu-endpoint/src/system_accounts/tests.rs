//! 装卸以后再建系统账号（施工 F-5 下）：开过索引的账号不再开，手里的还是同一份。

use std::sync::Arc;

use miyu_kernel::id::AccountId;
use miyu_session::testkit::Script;
use miyu_store::resources::ResourceRoot;
use miyu_tool::Catalog;

use crate::Core;
use crate::test_support::{resources, temp_root};

#[tokio::test]
async fn an_opened_index_is_kept_when_prepared_again() {
    let (root, dir) = temp_root("accounts");
    let core = Core::new(
        root,
        ResourceRoot::at(resources()),
        Arc::new(Script::new([])),
        Catalog::default(),
        None,
        AccountId::parse("alice").expect("合写法"),
        "token".to_string(),
    );
    let onebot = AccountId::parse("onebot").expect("合写法");
    super::prepare(&core);
    let first = core.index_for(&onebot).expect("出厂的接入QQ有系统账号");
    super::prepare(&core);
    let again = core.index_for(&onebot).expect("还在");
    assert!(Arc::ptr_eq(&first, &again), "开过的不再开");
    drop(core);
    if let Err(error) = std::fs::remove_dir_all(&dir) {
        eprintln!("临时目录没删掉：{error}");
    }
}
