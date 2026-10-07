//! 场所会话的测试用的（施工 O-3）：照主人对应表起来、带工具的核心。

use std::sync::Arc;

use miyu_endpoint::Core;
use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use super::{Home, TOKEN, alice};

/// 系统配置：`qq:10001` 是 alice 本人，`qq:10003` 对着一个不存在的账号。
pub const BINDINGS: &str =
    "[external.bindings]\n\"qq:10001\" = \"alice\"\n\"qq:10003\" = \"nobody\"\n";

/// 照对应表起来、工具目录是 `tools` 的核心：先把对应表写进系统配置。
pub fn bound_core(home: &Home, script: &Script, tools: Catalog) -> Arc<Core> {
    home.write("system/config.toml", BINDINGS);
    let items = [
        miyu_endpoint::settings::UiSettings::ITEMS,
        miyu_endpoint::settings::PermissionSettings::ITEMS,
        miyu_endpoint::settings::EXTERNAL_BINDINGS,
    ]
    .concat();
    let config = miyu_endpoint::config::Config::load(
        &home.root,
        &alice(),
        None,
        items,
        miyu_endpoint::config::Environment::of(&[]),
    );
    Arc::new(
        home.core_full(script, tools, None, TOKEN)
            .with_config(config),
    )
}
