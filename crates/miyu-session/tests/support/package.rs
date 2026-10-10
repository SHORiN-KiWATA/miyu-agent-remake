//! 本机 embedding 的测试共用的（施工 R-5 三补）：手造的小模型（`crates/miyu-embed/tests/fixtures/tiny/`）摆成一份「内置语义模型」
//! 的包目录（模型清单 `model.toml` 加两个文件，`docs/blueprint/recall.md` 第四条第 2 款），和 cargo 编出来的 `miyu-embed`。

use std::path::{Path, PathBuf};
use std::time::Duration;

use miyu_session::EmbedSetup;
use sha2::{Digest, Sha256};

/// 小模型的文件在哪（`miyu-embed` 的测试数据）。
pub fn tiny() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../miyu-embed/tests/fixtures/tiny")
}

/// cargo 编出来的 `miyu-embed`：和测试程序所在的 `deps/` 同一层（`cargo test --workspace` 会编它）。
pub fn program() -> PathBuf {
    let exe = std::env::current_exe().expect("知道测试程序在哪");
    let dir = exe
        .parent()
        .and_then(Path::parent)
        .expect("在 target/<profile>/deps/ 里");
    let program = dir.join(format!("miyu-embed{}", std::env::consts::EXE_SUFFIX));
    assert!(
        program.is_file(),
        "{} 不在：先 cargo build -p miyu-embed",
        program.display()
    );
    program
}

/// 在 `dir` 摆一份包目录：模型文件的内容是 `model`（小模型的原样，或者故意坏的），词表照小模型的，模型清单照这两份的大小、
/// SHA-256 写（不带 `url`）。交回照它接上要的，闲十分钟退出。
pub fn package(dir: &Path, model: &[u8]) -> EmbedSetup {
    std::fs::create_dir_all(dir).expect("建得了");
    let vocab = std::fs::read(tiny().join("vocab.txt")).expect("读得到");
    let text = format!(
        "id = \"tiny\"\ndims = 4\npooling = \"cls\"\nmax_tokens = 6\n\n\
         [[files]]\nrole = \"model\"\nname = \"model.onnx\"\nsha256 = \"{m}\"\nsize = {ms}\n\n\
         [[files]]\nrole = \"vocab\"\nname = \"vocab.txt\"\nsha256 = \"{v}\"\nsize = {vs}\n",
        m = hex(model),
        ms = model.len(),
        v = hex(&vocab),
        vs = vocab.len(),
    );
    std::fs::write(dir.join("model.toml"), text).expect("写得进");
    std::fs::write(dir.join("model.onnx"), model).expect("写得进");
    std::fs::write(dir.join("vocab.txt"), &vocab).expect("写得进");
    EmbedSetup {
        program: Some(program()),
        manifest: dir.join("model.toml"),
        dir: dir.to_path_buf(),
        idle: Duration::from_secs(600),
    }
}

/// 小模型原样摆在 `dir`。
pub fn tiny_package(dir: &Path) -> EmbedSetup {
    package(
        dir,
        &std::fs::read(tiny().join("model.onnx")).expect("读得到"),
    )
}

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
