//! 核心起来时把联想要的几样交给记忆（施工 R-8，`memory.md` 第四条第 7 款）：造好的核心再交一次交不进，就是交过了。

use std::sync::Arc;

use miyu_kernel::id::AccountId;
use miyu_session::RecallTexts;
use miyu_session::testkit::Script;
use miyu_store::resources::ResourceRoot;
use miyu_tool::Catalog;

use crate::Core;
use crate::test_support::{resources, temp_root};

#[test]
fn the_core_gives_recall_its_texts_when_it_starts() {
    let (root, _) = temp_root("recall");
    let resources = ResourceRoot::at(resources());
    let core = Core::new(
        root,
        resources.clone(),
        Arc::new(Script::new([])),
        Catalog::default(),
        None,
        AccountId::parse("admin").expect("合写法"),
        "token".to_string(),
    );
    let again = RecallTexts::load(&resources).expect("出厂的读得出来");
    assert!(!core.memory.give_recall(again), "起来时交过了");
}
