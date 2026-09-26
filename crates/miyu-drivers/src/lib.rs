//! 模型驱动的编码与解码（`docs/designs/05-内核接口.md` 第七节「驱动的规格」）：第 2 层的纯逻辑。
//!
//! 驱动是纯粹的翻译器：统一的请求编码成各家接口的请求字节，各家的响应解码成统一的增量，出错
//! 分成几类。真正发请求的是执行器。现在有的：
//!
//! - [`openai_chat`]：OpenAI 兼容的对话接口（DeepSeek、智谱、OpenRouter、本机的 Ollama 这些）的编码；
//! - [`base64`]：图片、文件写成 data URL 要用的编码。
//!
//! 一次调用要定的（[`Call`]）不在统一的请求里：同一份投影可以交给不同的端点。图片、文件的字节
//! 由执行器先从 blob 取出来交进来（[`BlobBytes`]），驱动不碰文件；给模型看的几句占位也由执行器
//! 从资源目录读好交进来（[`DriverTexts`]）。

pub mod base64;
pub mod openai_chat;
mod texts;

pub use texts::{DriverTextSources, DriverTexts};

use std::collections::BTreeMap;

use miyu_kernel::id::{ContentHash, ModelName};

/// 一次调用要定的：发给哪个模型、输出的上限、模型能收哪些输入。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    /// 模型名，照供应商那边的叫法。
    pub model: ModelName,
    /// 最多输出多少 token。没有就不写，照供应商的默认。
    pub max_output: Option<u32>,
    /// 模型能收哪些输入。
    pub inputs: Inputs,
}

/// 模型能收哪些输入：模型资料（`15-模型与供应商.md` 第三节）。查不到的都当不能收，这是驱动的
/// 保守默认。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Inputs {
    /// 能看图。
    pub images: bool,
    /// 能读 PDF。
    pub pdf: bool,
}

/// 取 blob 字节的端口：执行器照驱动列出的清单先取出来，编码时交进来。
pub trait BlobBytes {
    /// 这个 blob 的字节；没有，返回 `None`。
    fn bytes(&self, hash: &ContentHash) -> Option<&[u8]>;
}

impl BlobBytes for BTreeMap<ContentHash, Vec<u8>> {
    fn bytes(&self, hash: &ContentHash) -> Option<&[u8]> {
        self.get(hash).map(Vec::as_slice)
    }
}
