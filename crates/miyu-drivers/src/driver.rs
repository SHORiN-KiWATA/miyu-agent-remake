//! 驱动的接口（`docs/designs/05-内核接口.md` 第七节「驱动的规格」）：执行器照着它调，不用知道是
//! 哪一家。编码、解码、分类都是纯函数；真正发请求的是执行器。
//!
//! 认证头（[`Driver::auth`]）随施工 8-6；列模型（[`Driver::models_path`]、[`Driver::parse_models`]）随施工 8-7；`cache`
//! （缓存类型）随用到它的那一步；`transport`
//! 现在只有 HTTP，发到哪条路径跟着编码结果走（[`Encoded::path`]，施工 3-5 再补）。

use std::collections::BTreeSet;

use miyu_kernel::accumulate::Delta;
use miyu_kernel::id::ContentHash;
use miyu_kernel::request::Request;

use crate::classify::{self, Classified, Failure};
use crate::openai_chat::{self, Compat, Decoder};
use crate::{BlobBytes, Call, DriverTexts, EncodeError, Encoded, Ending, Listed};
use crate::{anthropic, openai_responses};

/// 一个驱动：一家接口的翻译器。一个会话造一个，开关和占位冻结在里面。执行器在异步任务里用它，
/// 所以能跨线程。
pub trait Driver: Send + Sync {
    /// 驱动家族：私有数据里写的是它的，才归它用（`03-事件模型.md` 第九节）。
    fn family(&self) -> &'static str;

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

    /// 带 key `key` 的请求要带的认证头，照先后（`models.md`「驱动要守的约定」第 2 条，施工 8-6）：HTTP 执行器照它写，
    /// 不自己写 `Bearer`。没有 key 的请求（本机的服务）不问它，什么都不带。
    fn auth(&self, key: &str) -> Vec<(String, String)>;

    /// 列模型发到地址后面的哪一截（`models.md`「驱动要守的约定」第 3 条，施工 8-7）：执行器照它 GET，带认证头。
    fn models_path(&self) -> &'static str;

    /// 读列模型的回应：模型名和报了的窗口，分页照那一家的。
    ///
    /// # Errors
    ///
    /// 回应不是那一家的写法：原话说是哪一种。
    fn parse_models(&self, bytes: &[u8]) -> Result<Vec<Listed>, String>;
}

/// 一次响应的解码器。读流跨过好几次等待，所以能跨线程。
pub trait Decode: Send {
    /// 喂一片字节，交回解出来的增量。
    fn feed(&mut self, bytes: &[u8]) -> Vec<Delta>;

    /// 不用再读了。
    fn done(&self) -> bool;

    /// `finish_reason` 到了：模型说完了，只差流的结尾（施工 4-9 再补三下）。这时停住不动的，执行器当说完了。
    fn finished(&self) -> bool;

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

    /// `Authorization: Bearer <key>`。
    fn auth(&self, key: &str) -> Vec<(String, String)> {
        vec![("Authorization".to_string(), format!("Bearer {key}"))]
    }

    /// `/models`。
    fn models_path(&self) -> &'static str {
        openai_chat::MODELS_PATH
    }

    fn parse_models(&self, bytes: &[u8]) -> Result<Vec<Listed>, String> {
        openai_chat::parse_models(bytes)
    }
}

impl Decode for Decoder {
    fn feed(&mut self, bytes: &[u8]) -> Vec<Delta> {
        Decoder::feed(self, bytes)
    }

    fn done(&self) -> bool {
        Decoder::done(self)
    }

    fn finished(&self) -> bool {
        Decoder::finished(self)
    }

    fn finish(self: Box<Self>) -> Ending {
        Decoder::finish(*self)
    }
}

/// Anthropic 的消息接口（施工 8-12，`docs/blueprint/drivers/anthropic.md`）：写法只有一套，没有开关，只带占位的几句。
#[derive(Debug, Clone)]
pub struct Anthropic {
    texts: DriverTexts,
}

impl Anthropic {
    /// 照会话冻结的占位造一个。
    pub fn new(texts: DriverTexts) -> Anthropic {
        Anthropic { texts }
    }
}

impl Driver for Anthropic {
    fn family(&self) -> &'static str {
        anthropic::FAMILY
    }

    fn blobs_needed(&self, request: &Request, call: &Call) -> BTreeSet<ContentHash> {
        anthropic::blobs_needed(request, call)
    }

    fn encode(
        &self,
        request: &Request,
        call: &Call,
        blobs: &dyn BlobBytes,
    ) -> Result<Encoded, EncodeError> {
        anthropic::encode(request, call, &self.texts, blobs)
    }

    fn decoder(&self) -> Box<dyn Decode> {
        Box::new(anthropic::Decoder::new())
    }

    fn classify(&self, failure: &Failure<'_>) -> Classified {
        classify::classify(failure)
    }

    /// `x-api-key: <key>`、`anthropic-version: 2023-06-01`，照这个先后。
    fn auth(&self, key: &str) -> Vec<(String, String)> {
        vec![
            ("x-api-key".to_string(), key.to_string()),
            (
                "anthropic-version".to_string(),
                anthropic::VERSION.to_string(),
            ),
        ]
    }

    /// `/models?limit=1000`。
    fn models_path(&self) -> &'static str {
        anthropic::MODELS_PATH
    }

    fn parse_models(&self, bytes: &[u8]) -> Result<Vec<Listed>, String> {
        anthropic::parse_models(bytes)
    }
}

impl Decode for anthropic::Decoder {
    fn feed(&mut self, bytes: &[u8]) -> Vec<Delta> {
        anthropic::Decoder::feed(self, bytes)
    }

    fn done(&self) -> bool {
        anthropic::Decoder::done(self)
    }

    fn finished(&self) -> bool {
        anthropic::Decoder::finished(self)
    }

    fn finish(self: Box<Self>) -> Ending {
        anthropic::Decoder::finish(*self)
    }
}

/// OpenAI 的 Responses 接口（施工 8-13，`docs/blueprint/drivers/openai-responses.md`）：写法只有一套，没有开关，只带占位的几句。
#[derive(Debug, Clone)]
pub struct OpenAiResponses {
    texts: DriverTexts,
}

impl OpenAiResponses {
    /// 照会话冻结的占位造一个。
    pub fn new(texts: DriverTexts) -> OpenAiResponses {
        OpenAiResponses { texts }
    }
}

impl Driver for OpenAiResponses {
    fn family(&self) -> &'static str {
        openai_responses::FAMILY
    }

    fn blobs_needed(&self, request: &Request, call: &Call) -> BTreeSet<ContentHash> {
        openai_responses::blobs_needed(request, call)
    }

    fn encode(
        &self,
        request: &Request,
        call: &Call,
        blobs: &dyn BlobBytes,
    ) -> Result<Encoded, EncodeError> {
        openai_responses::encode(request, call, &self.texts, blobs)
    }

    fn decoder(&self) -> Box<dyn Decode> {
        Box::new(openai_responses::Decoder::new())
    }

    fn classify(&self, failure: &Failure<'_>) -> Classified {
        classify::classify(failure)
    }

    /// `Authorization: Bearer <key>`。
    fn auth(&self, key: &str) -> Vec<(String, String)> {
        vec![("Authorization".to_string(), format!("Bearer {key}"))]
    }

    /// `/models`：和 openai-chat 一样。
    fn models_path(&self) -> &'static str {
        openai_chat::MODELS_PATH
    }

    fn parse_models(&self, bytes: &[u8]) -> Result<Vec<Listed>, String> {
        openai_chat::parse_models(bytes)
    }
}

impl Decode for openai_responses::Decoder {
    fn feed(&mut self, bytes: &[u8]) -> Vec<Delta> {
        openai_responses::Decoder::feed(self, bytes)
    }

    fn done(&self) -> bool {
        openai_responses::Decoder::done(self)
    }

    fn finished(&self) -> bool {
        openai_responses::Decoder::finished(self)
    }

    fn finish(self: Box<Self>) -> Ending {
        openai_responses::Decoder::finish(*self)
    }
}
