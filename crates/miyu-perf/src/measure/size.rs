//! 二进制大小：主程序和它旁边的沙盒助手。没有预算（23「后续再定」：发行构建怎么配，原型阶段按体积和速度实测定），
//! 记下来给那一条用。strip 以后的照拷一份再 `strip` 量；没有 `strip` 的平台不写。

use std::path::Path;
use std::process::Command;

use serde_json::{Value, json};

/// 一个程序多大。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Size {
    /// 名字。
    pub name: String,
    /// 字节数。
    pub bytes: u64,
    /// strip 以后的字节数；量不了的没有。
    pub stripped: Option<u64>,
}

impl Size {
    /// 写进原始数据的样子。
    pub fn to_json(&self) -> Value {
        json!({"name": self.name, "bytes": self.bytes, "stripped": self.stripped})
    }
}

/// `miyu` 和它旁边的 `miyu-sandbox`。拷贝放在 `scratch` 下。没有的程序不列。
pub fn sizes(miyu: &Path, scratch: &Path) -> Vec<Size> {
    ["miyu", "miyu-sandbox"]
        .iter()
        .filter_map(|name| {
            let file = format!("{name}{}", std::env::consts::EXE_SUFFIX);
            let path = miyu.with_file_name(&file);
            let bytes = std::fs::metadata(&path).ok()?.len();
            Some(Size {
                name: (*name).to_string(),
                bytes,
                stripped: stripped(&path, &scratch.join(&file)),
            })
        })
        .collect()
}

/// 拷一份到 `copy`，strip 它，交回大小。
fn stripped(path: &Path, copy: &Path) -> Option<u64> {
    std::fs::create_dir_all(copy.parent()?).ok()?;
    std::fs::copy(path, copy).ok()?;
    let status = Command::new("strip").arg(copy).status().ok()?;
    if !status.success() {
        return None;
    }
    Some(std::fs::metadata(copy).ok()?.len())
}
