use std::sync::Arc;

use miyu_kernel::tool::Access;

use super::*;
use crate::testkit::{Act, Fake};

fn object(name: &str) -> Arc<dyn Tool> {
    Fake::new(name, Access::Read, Act::Echo)
}

fn names(catalog: &Catalog) -> Vec<String> {
    catalog.specs().map(|spec| spec.name.clone()).collect()
}

/// 换一次是一代：拿着同一个架子的都看到新的；换不成的不动、不算一代。
#[test]
fn a_shelf_hands_out_the_current_catalog_and_counts_editions() {
    let shelf = Shelf::new(Catalog::new([object("read")]).unwrap());
    let held = shelf.clone();
    let first = shelf.edition();
    assert_eq!(names(&first.catalog), ["read"]);
    let generation = shelf
        .replace(|catalog| catalog.replacing("onebot", vec![object("send")]))
        .unwrap();
    assert_eq!(generation, first.generation + 1);
    assert_eq!(
        names(&held.current()),
        ["read", "send"],
        "拿着同一个架子的看到新的"
    );
    assert!(held.get("send").is_some());
    assert!(held.get("nothing").is_none());
    let refused = shelf.replace(|catalog| catalog.replacing("onebot", vec![object("read")]));
    assert!(refused.is_err());
    let now = shelf.edition();
    assert_eq!(now.generation, generation, "换不成的不算一代");
    assert_eq!(names(&now.catalog), ["read", "send"], "换不成的不动");
}
