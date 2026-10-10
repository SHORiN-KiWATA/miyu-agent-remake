//! 扩展进程调了回 `local_only` 的方法（施工 9-4 上、P-3 中、T-9）：真起一个扩展进程的那一条在 `tests/extensions.rs`，这里守名单。

use super::people_only;

#[test]
fn extensions_cannot_switch_extensions_or_write_personas_and_presets() {
    for method in [
        "extension.enable",
        "extension.restart",
        "preset.set",
        "preset.delete",
        "persona.set",
        "persona.delete",
        "package.install",
        "package.remove",
        "package.enable",
        "package.disable",
        "package.file",
        "package.call",
        "config.set",
        "config.trust",
        "secret.set",
        "secret.delete",
    ] {
        assert!(people_only(method), "{method}");
    }
    for method in [
        "preset.get",
        "persona.get",
        "persona.read",
        "session.send",
        "package.list",
        "config.get",
        "config.schema",
        "config.check",
        "secret.list",
    ] {
        assert!(!people_only(method), "{method}");
    }
}
