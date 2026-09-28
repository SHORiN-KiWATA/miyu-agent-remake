use serde_json::json;

use super::{Package, allowed, judge, third_party};

#[test]
fn expressions_count_as_spdx_says() {
    // 仓库里现在出现过的写法都能用。
    for ok in [
        "MIT",
        "MIT OR Apache-2.0",
        "Apache-2.0 OR MIT",
        "MIT/Apache-2.0",
        "Apache-2.0 / MIT",
        "Unlicense/MIT",
        "Apache-2.0 OR ISC OR MIT",
        "Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT",
        "(Apache-2.0 OR MIT) AND BSD-3-Clause",
        "(MIT OR Apache-2.0) AND Unicode-3.0",
        "Apache-2.0 AND ISC",
        "Zlib OR Apache-2.0 OR MIT",
        "CDLA-Permissive-2.0",
        "MIT or Apache-2.0",
        "Apache-2.0+",
    ] {
        assert!(allowed(ok), "{ok}");
    }
    // OR 有一个能用就行；AND 每个都要能用；WITH 后面只认 LLVM-exception。
    assert!(allowed("GPL-2.0-only OR MIT"));
    for bad in [
        "GPL-2.0-only",
        "MIT AND GPL-2.0-only",
        "(MIT OR Apache-2.0) AND GPL-2.0-only",
        "Apache-2.0 WITH Classpath-exception-2.0",
        "SSPL-1.0",
    ] {
        assert!(!allowed(bad), "{bad}");
    }
    // 写坏了的当不能用。
    for broken in [
        "", "(MIT", "MIT)", "MIT OR", "AND MIT", "MIT WITH", "MIT MIT",
    ] {
        assert!(!allowed(broken), "{broken:?}");
    }
}

/// 一个包。
fn package(name: &str, license: Option<&str>) -> Package {
    Package {
        name: name.to_string(),
        version: "1.0.0".to_string(),
        license: license.map(str::to_string),
    }
}

#[test]
fn each_bad_package_is_named_once_with_its_platforms() {
    let gpl = package("gpl-thing", Some("GPL-2.0-only"));
    let bare = package("bare", None);
    let fine = package("fine", Some("MIT OR Apache-2.0"));
    let found = [
        (gpl.clone(), "x86_64-unknown-linux-gnu"),
        (gpl, "aarch64-apple-darwin"),
        (bare, "x86_64-pc-windows-msvc"),
        (fine, "x86_64-unknown-linux-gnu"),
    ];
    assert_eq!(
        judge(&found),
        [
            "bare 1.0.0（x86_64-pc-windows-msvc）：没写 license，要人看过",
            "gpl-thing 1.0.0（aarch64-apple-darwin、x86_64-unknown-linux-gnu）：GPL-2.0-only 和 GPL-3.0-or-later 合不到一起",
        ]
    );
}

#[test]
fn only_third_party_packages_in_the_graph_are_looked_at() {
    let metadata = json!({
        "workspace_members": ["path+file:///repo#miyu@0.0.0"],
        "resolve": {"nodes": [
            {"id": "path+file:///repo#miyu@0.0.0"},
            {"id": "registry+https://github.com/rust-lang/crates.io-index#serde@1.0.0"},
        ]},
        "packages": [
            {"id": "path+file:///repo#miyu@0.0.0", "name": "miyu", "version": "0.0.0", "license": "GPL-3.0-or-later"},
            {"id": "registry+https://github.com/rust-lang/crates.io-index#serde@1.0.0", "name": "serde", "version": "1.0.0", "license": "MIT OR Apache-2.0"},
            {"id": "registry+https://github.com/rust-lang/crates.io-index#winapi@0.3.9", "name": "winapi", "version": "0.3.9", "license": "MIT/Apache-2.0"},
            {"id": "registry+https://github.com/rust-lang/crates.io-index#nolicense@1.0.0", "name": "nolicense", "version": "1.0.0", "license": null},
        ],
    });
    assert_eq!(
        third_party(&metadata),
        [Package {
            name: "serde".to_string(),
            version: "1.0.0".to_string(),
            license: Some("MIT OR Apache-2.0".to_string()),
        }],
        "工作区自己的不看；依赖图里没有的（别的平台才用的）不看"
    );
}
