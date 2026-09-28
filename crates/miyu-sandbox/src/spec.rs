//! 规格（`docs/blueprint/sandbox.md`「对外的样子」）：哪些能读，哪些能写，能写的里面哪些只能读，哪些藏起来，网络怎么走。
//! 路径都是真实的位置。写成 JSON 经命令行交给助手，里面只有路径和网络怎么走，没有密钥。

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// 一条命令关进沙盒的规格。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    /// 能读的目录、文件。
    #[serde(default)]
    pub read: Vec<PathBuf>,
    /// 能读能写的。
    #[serde(default)]
    pub write: Vec<PathBuf>,
    /// 能写的那几片里只能读的，例如工作区的 `.git/hooks`、`.git/config`。
    #[serde(default)]
    pub readonly: Vec<PathBuf>,
    /// 读写都不行、要藏起来的，例如数据根：它可能落在能写的临时目录里。
    #[serde(default)]
    pub hidden: Vec<PathBuf>,
    /// 网络怎么走。
    pub network: Network,
}

/// 沙盒里的网络。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Network {
    /// 不能联网。
    Off,
    /// 只能连 Miyu 的代理，例如 `127.0.0.1:41234`。
    Proxy(String),
}

impl Spec {
    /// 写成一行 JSON：助手的 `--spec` 收的就是它。
    ///
    /// # Errors
    ///
    /// 实际不会出错：几格都是路径和字符串。路径不是 UTF-8 的，serde 交回错误。
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// 从 JSON 读回来。
    ///
    /// # Errors
    ///
    /// 不是 JSON、少了 `network`、有认不得的格、类型不对。
    pub fn from_json(text: &str) -> Result<Spec, serde_json::Error> {
        serde_json::from_str(text)
    }
}

#[cfg(test)]
mod tests;
