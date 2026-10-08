//! `compaction.prepare`：默认开，系统配置、个人设置两层能写，项目配置不能，下一轮生效。

use super::*;
use miyu_config::{Applies, Layer, Values};

#[test]
fn preparing_is_on_by_default_and_applies_next_turn() {
    let defaults = CompactionSettings::from(&Values::defaults(CompactionSettings::ITEMS));
    assert!(defaults.prepare);
    let item = &CompactionSettings::ITEMS[0];
    assert_eq!(item.key, "compaction.prepare");
    assert_eq!(item.layers, &[Layer::System, Layer::Personal]);
    assert_eq!(item.applies, Applies::NextTurn);
}
