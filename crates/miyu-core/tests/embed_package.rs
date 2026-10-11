//! 内置语义模型这个小程序包（施工 R-5 三补，设计 `30-插件框架.md` 第六节）：仓库里留的原本读得成小程序包、程序是
//! `miyu-embed`；出厂的人格记忆推荐它。模型清单（`package/embed/model.toml`）读得成，在 `miyu-embed` 的测试里。

use std::path::PathBuf;

use miyu_config::package::{PackageKind, read};

use crate::support::resources;

#[test]
fn the_embed_package_is_a_worker_that_memory_recommends() {
    let original =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../miyu-embed/package/embed/package.toml");
    let embed = read(&std::fs::read_to_string(&original).expect("有原本")).expect("写法对");
    assert_eq!(embed.kind, PackageKind::Worker);
    assert_eq!(
        embed.worker.as_ref().map(|worker| worker.program.as_str()),
        Some("miyu-embed")
    );
    let memory = resources()
        .path()
        .join("packages")
        .join("memory")
        .join("package.toml");
    let memory = read(&std::fs::read_to_string(&memory).expect("出厂有")).expect("写法对");
    assert_eq!(memory.recommends, ["embed"]);
    assert!(memory.depends.is_empty(), "推荐，不是缺了不起");
    assert!(
        !resources().path().join("packages").join("embed").exists(),
        "出厂不装"
    );
}
