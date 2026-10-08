//! 测试用的人格样本（施工 P-4 下：出厂不带人格）：`docs/designs/samples/personas/engineer/`，原来出厂的软件工程师。要人格的
//! 测试把它装进数据根的系统区那一层，照旧用编号 `engineer`；系统区的人格记忆归会话的属主，和原来出厂的一样。

use std::path::{Path, PathBuf};

use miyu_policy::PersonaTexts;
use miyu_store::root::DataRoot;

/// 样本人格的编号。
pub const SAMPLE_PERSONA: &str = "engineer";

/// 样本人格的目录。
pub fn sample_persona_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/designs/samples/personas/engineer")
}

/// 样本的字：人设那一句，没有示范对话、角色扮演提示。
///
/// # Panics
///
/// 样本读不出来：仓库坏了。
pub fn sample_persona_texts() -> PersonaTexts {
    let persona = std::fs::read_to_string(sample_persona_dir().join("prompts/persona.md"))
        .unwrap_or_else(|error| panic!("样本人格读得出来：{error}"));
    PersonaTexts {
        persona,
        ..PersonaTexts::default()
    }
}

/// 把样本装进数据根 `root` 的系统区那一层：`system/personas/engineer/`。
///
/// # Panics
///
/// 写不进临时数据根。
pub fn install_sample_persona(root: &DataRoot) {
    copy(
        &sample_persona_dir(),
        &root.system().join("personas").join(SAMPLE_PERSONA),
    );
}

/// 整个目录照样复制一份。
fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap_or_else(|error| panic!("建得了 {}：{error}", to.display()));
    let entries = std::fs::read_dir(from).unwrap_or_else(|error| panic!("读得了样本：{error}"));
    for entry in entries.flatten() {
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target)
                .unwrap_or_else(|error| panic!("复制得了 {}：{error}", target.display()));
        }
    }
}
