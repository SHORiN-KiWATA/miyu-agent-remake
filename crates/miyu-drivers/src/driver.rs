//! 驱动的接口（`docs/designs/05-内核接口.md` 第七节「驱动的规格」）：执行器照着它调，不用知道是
//! 哪一家。编码、解码、分类都是纯函数；真正发请求的是执行器。
//!
//! 规格里的 `models`（模型资料）、`cache`（缓存类型）到配置和模型资料的那几步再加；`transport`
//! 现在只有 HTTP，就是这里的路径。

use std::collections::BTreeSet;

use miyu_kernel::accumulate::Delta;
use miyu_kernel::id::ContentHash;
use miyu_kernel::request::Request;

use crate::classify::{self, Classified, Failure};
use crate::openai_chat::{self, Compat, Decoder};
use crate::{BlobBytes, Call, DriverTexts, EncodeError, Encoded, Ending};

/// 一个驱动：一家接口的翻译器。一个会话造一个，开关和占位冻结在里面。执行器在异步任务里用它，
/// 所以能跨线程。
pub trait Driver: Send + Sync {
    /// 驱动家族：私有数据里写的是它的，才归它用（`03-事件模型.md` 第九节）。
    fn family(&self) -> &'static str;

    /// 请求发到供应商地址后面的这一截。
    fn path(&self) -> &'static str;

    /// 这份请求编码时要用哪些 blob，执行器照着先取出来。
    fn blobs_needed(&self, request: &Request, call: &Call) -> BTreeSet<ContentHash>;

    /// 编码。
    ///
    /// # Errors
    ///
    /// 要用的 blob 执行器没交进来。
    fn encode(
        &self,
        request: &Request,
        call: &Call,
        blobs: &dyn BlobBytes,
    ) -> Result<Encoded, EncodeError>;

    /// 一次响应一个解码器。
    fn decoder(&self) -> Box<dyn Decode>;

    /// 出错分类。
    fn classify(&self, failure: &Failure<'_>) -> Classified;
}

/// 一次响应的解码器。读流跨过好几次等待，所以能跨线程。
pub trait Decode: Send {
    /// 喂一片字节，交回解出来的增量。
    fn feed(&mut self, bytes: &[u8]) -> Vec<Delta>;

    /// 不用再读了。
    fn done(&self) -> bool;

    /// 流完了，或者不再读了：收块的增量、用量、出错。
    fn finish(self: Box<Self>) -> Ending;
}

/// OpenAI 兼容的对话接口。
#[derive(Debug, Clone)]
pub struct OpenAiChat {
    compat: Compat,
    texts: DriverTexts,
}

impl OpenAiChat {
    /// 照供应商的开关、会话冻结的占位造一个。
    pub fn new(compat: Compat, texts: DriverTexts) -> OpenAiChat {
        OpenAiChat { compat, texts }
    }
}

impl Driver for OpenAiChat {
    fn family(&self) -> &'static str {
        openai_chat::FAMILY
    }

    fn path(&self) -> &'static str {
        openai_chat::PATH
    }

    fn blobs_needed(&self, request: &Request, call: &Call) -> BTreeSet<ContentHash> {
        openai_chat::blobs_needed(request, call)
    }

    fn encode(
        &self,
        request: &Request,
        call: &Call,
        blobs: &dyn BlobBytes,
    ) -> Result<Encoded, EncodeError> {
        openai_chat::encode(request, call, &self.compat, &self.texts, blobs)
    }

    fn decoder(&self) -> Box<dyn Decode> {
        Box::new(Decoder::new())
    }

    fn classify(&self, failure: &Failure<'_>) -> Classified {
        classify::classify(failure)
    }
}

impl Decode for Decoder {
    fn feed(&mut self, bytes: &[u8]) -> Vec<Delta> {
        Decoder::feed(self, bytes)
    }

    fn done(&self) -> bool {
        Decoder::done(self)
    }

    fn finish(self: Box<Self>) -> Ending {
        Decoder::finish(*self)
    }
}
