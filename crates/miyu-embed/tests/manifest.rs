//! 清单（`recall.md` 第四条第 2 款）：出厂那一份读得出；少格、多格、`role` 重了或缺了、不认识的取法、数不对的都拒，说清楚
//! 哪里不对。

use miyu_embed::manifest::{Manifest, Pooling, Role};

use crate::support::shipped_manifest;

/// 一份合写法的清单，`extra` 接在最上面那几格后面，`files` 是 `[[files]]` 那几段。
fn manifest(top: &str, files: &str) -> String {
    format!("id = \"m\"\ndims = 4\npooling = \"cls\"\nmax_tokens = 8\n{top}\n{files}")
}

const BOTH: &str = "[[files]]\nrole = \"model\"\nname = \"model.onnx\"\nurl = \"https://example.invalid/a\"\nsha256 = \"00\"\nsize = 1\n\n[[files]]\nrole = \"vocab\"\nname = \"vocab.txt\"\nurl = \"https://example.invalid/b\"\nsha256 = \"11\"\nsize = 2\n";

#[test]
fn the_shipped_one_reads() {
    let manifest = Manifest::read(&shipped_manifest()).expect("出厂的读得出");
    assert_eq!(manifest.id, "bge-small-zh-v1.5");
    assert_eq!(manifest.model(), "local:bge-small-zh-v1.5");
    assert_eq!(manifest.dims, 512);
    assert_eq!(manifest.pooling, Pooling::Cls);
    assert_eq!(manifest.max_tokens, 512);
    let model = manifest.file(Role::Model);
    assert_eq!(model.name, "model_quantized.onnx");
    assert_eq!(model.size, 24_010_842);
    assert!(
        model.url.starts_with("https://github.com/"),
        "{}",
        model.url
    );
    assert_eq!(manifest.file(Role::Vocab).name, "vocab.txt");
}

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

#[test]
fn a_missing_file_says_where() {
    let path = std::path::Path::new("/nonexistent/manifest.toml");
    let error = Manifest::read(path).expect_err("没有这个文件").to_string();
    assert!(
        error.starts_with("cannot read /nonexistent/manifest.toml"),
        "{error}"
    );
}
