//! 读清单的文件（`recall.md` 第四条第 2 款）：内置语义模型的包里那一份读得出、每一格对、没有下载地址（R-5 三补）；文件读不了
//! 说是哪个。原文怎么读、怎么查在
//! `miyu_recall::embedding` 的单元测试里。

use miyu_embed::manifest::{self, Pooling, Role};

use crate::support::shipped_manifest;

#[test]
fn the_shipped_one_reads() {
    let manifest = manifest::read(&shipped_manifest()).expect("出厂的读得出");
    assert_eq!(manifest.id, "bge-small-zh-v1.5");
    assert_eq!(manifest.model(), "local:bge-small-zh-v1.5");
    assert_eq!(manifest.dims, 512);
    assert_eq!(manifest.pooling, Pooling::Cls);
    assert_eq!(manifest.max_tokens, 512);
    let model = manifest.file(Role::Model);
    assert_eq!(model.name, "model_quantized.onnx");
    assert_eq!(model.size, 24_010_842);
    assert_eq!(
        model.sha256, "15b717c382bcb518ba457b93ea6850ede7f4f1cd8937454aa06972366cd19bcc",
        "和 Release 的 SHA256SUMS 一样"
    );
    assert_eq!(manifest.file(Role::Vocab).name, "vocab.txt");
}

#[test]
fn a_missing_file_says_where() {
    let path = std::path::Path::new("/nonexistent/manifest.toml");
    let error = manifest::read(path).expect_err("没有这个文件");
    assert!(
        error.starts_with("cannot read /nonexistent/manifest.toml"),
        "{error}"
    );
}
