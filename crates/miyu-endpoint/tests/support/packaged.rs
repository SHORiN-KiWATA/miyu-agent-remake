//! 照 `miyu-core` 起来时那样造的核心：清单读一次，照核心自己的几项 `settle`，包的配置项拼进配置清单（施工 9-1 下）。

use std::sync::Arc;

use miyu_endpoint::Core;
use miyu_endpoint::config::{Config, Environment};
use miyu_session::testkit::Script;

use super::{Home, TOKEN, default_resources};

/// 一份核心：数据根里这时有的包都读进来，它们的配置项在配置清单里。
pub fn core(home: &Home) -> Arc<Core> {
    let core_items = [
        miyu_endpoint::settings::UiSettings::ITEMS,
        miyu_endpoint::settings::PersonaSettings::ITEMS,
        miyu_endpoint::settings::PresetSettings::ITEMS,
        miyu_endpoint::settings::PermissionSettings::ITEMS,
    ]
    .concat();
    let resources = miyu_store::resources::ResourceRoot::at(default_resources());
    let alice = miyu_kernel::id::AccountId::parse("alice").expect("账号合写法");
    let mut found = miyu_endpoint::packages::load(&resources, &home.root, &alice);
    let packaged = miyu_endpoint::packages::settle(&mut found, &core_items);
    let items = core_items.into_iter().chain(packaged).collect();
    let config = Config::load(&home.root, &alice, None, items, Environment::of(&[]));
    Arc::new(
        home.core_full(&Script::new([]), miyu_tool::Catalog::default(), None, TOKEN)
            .with_config(config)
            .with_packages(found),
    )
}
