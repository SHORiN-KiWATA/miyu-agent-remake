//! 清单的读法（`recall.md` 第四条第 2 款）：合写法的读得出；少格、多格、`role` 重了或缺了、不认识的取法、数不对、文件名带
//! 目录、不是 TOML 的都拒，说清楚哪里不对。出厂那一份读不读得出在 `miyu-embed` 的测试里（要读文件）。

use super::*;

/// 一份合写法的清单，`top` 接在最上面那几格后面，`files` 是 `[[files]]` 那几段。
fn manifest(top: &str, files: &str) -> String {
    format!("id = \"m\"\ndims = 4\npooling = \"cls\"\nmax_tokens = 8\n{top}\n{files}")
}

const BOTH: &str = "[[files]]\nrole = \"model\"\nname = \"model.onnx\"\nurl = \"https://example.invalid/a\"\nsha256 = \"00\"\nsize = 1\n\n[[files]]\nrole = \"vocab\"\nname = \"vocab.txt\"\nurl = \"https://example.invalid/b\"\nsha256 = \"11\"\nsize = 2\n";

#[test]
fn a_good_one_parses() {
    let manifest = Manifest::parse(&manifest("", BOTH)).expect("合写法");
    assert_eq!(manifest.file(Role::Vocab).sha256, "11");
    assert_eq!(manifest.files.len(), 2);
}

#[test]
fn what_is_wrong_is_said() {
    let wrong = [
        (
            manifest("", "").replace("dims = 4\n", ""),
            "missing field `dims`",
        ),
        (manifest("color = \"red\"", BOTH), "unknown field `color`"),
        (
            manifest("", BOTH).replace("pooling = \"cls\"", "pooling = \"mean\""),
            "unknown variant `mean`",
        ),
        (
            manifest("", BOTH).replace("role = \"vocab\"", "role = \"model\""),
            "two files are model",
        ),
        (
            manifest("", &BOTH[..BOTH.find("\n\n").expect("两段")]),
            "no file is vocab",
        ),
        (
            manifest("", BOTH).replace("dims = 4", "dims = 0"),
            "dims must be at least 1",
        ),
        (
            manifest("", BOTH).replace("max_tokens = 8", "max_tokens = 1"),
            "max_tokens must be at least 2",
        ),
        (
            manifest("", BOTH).replace("name = \"vocab.txt\"", "name = \"../vocab.txt\""),
            "file name ../vocab.txt is not a plain name",
        ),
        (
            manifest("", BOTH).replace("id = \"m\"", "id = \"../m\""),
            "id ../m is not a plain name",
        ),
        ("not toml [".to_string(), "invalid"),
    ];
    for (text, said) in wrong {
        let error = Manifest::parse(&text).expect_err(said).to_string();
        assert!(
            error.contains(said),
            "说的是「{error}」，要有「{said}」：\n{text}"
        );
    }
}
