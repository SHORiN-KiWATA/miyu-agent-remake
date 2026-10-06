//! 图片、文件发不了时换成的字（`docs/blueprint/drivers/openai-chat.md` 第 9 条，施工 8-12 从 `openai_chat/messages.rs`
//! 挪出来，几个驱动共用，`docs/blueprint/drivers/anthropic.md`「在哪」）：
//!
//! - 不能看图的：有转述、快照里有那三句标签的写成带标签的转述（施工 8-17），别的写占位那一句，带名字的写上名字；
//! - 能看图的、带名字的图片前后各一段标签（施工 3-9 四补）；
//! - 发不了的文件：内容是文本的照字放进来，带着文件名（施工 3-9 三补，[`crate::text_file`]）；别的写一句占位，带文件名、
//!   媒体类型、大小。
//!
//! 字节由执行器先取好交进来（[`BlobBytes`]），没交进来的报缺了哪一个；要哪些照 [`blobs_needed`]。

use std::collections::{BTreeMap, BTreeSet};

use miyu_kernel::block::{Block, File, Image};
use miyu_kernel::id::ContentHash;
use miyu_kernel::request::{Message, Request};

use crate::{BlobBytes, Call, DriverTexts, EncodeError, text_file};

/// 换成字要用的：占位的几句、blob 的字节、请求里的图的转述。
pub(crate) struct Media<'a> {
    /// 会话冻结的占位。
    pub(crate) texts: &'a DriverTexts,
    /// 执行器先取好的字节。
    pub(crate) blobs: &'a dyn BlobBytes,
    /// 请求里的图的转述（施工 8-17）：不能看图时照它写。
    pub(crate) described: &'a BTreeMap<ContentHash, String>,
}

impl Media<'_> {
    /// 不能看图时的一张图：有转述、快照里有标签的写成带标签的转述（施工 8-17），别的写占位那一句。
    pub(crate) fn unseen(&self, image: &Image) -> String {
        self.described
            .get(&image.blob)
            .and_then(|description| self.texts.image_described(name(image), description))
            .unwrap_or_else(|| self.texts.image_omitted(name(image)))
    }

    /// 能看图时，带名字的图片前后的两段标签；不带名字的、快照里没有这几句的没有。
    pub(crate) fn image_tags(&self, image: &Image) -> Option<(String, String)> {
        self.texts.image_tags(name(image))
    }

    /// 发不了的文件写成字：内容是文本的、快照里有那三句的，照字放进来；别的写一句占位，带文件名、媒体类型、大小。
    pub(crate) fn file_text(&self, file: &File) -> Result<String, EncodeError> {
        let bytes = self.bytes(&file.blob)?;
        let name = file.name.as_str();
        let text = text_file::as_text(bytes).and_then(|text| self.texts.text_file(name, text));
        Ok(text.unwrap_or_else(|| {
            self.texts
                .file_omitted(name, file.media_type.as_str(), bytes.len())
        }))
    }

    /// 执行器先取好的字节；没交进来的报缺了哪一个。
    pub(crate) fn bytes(&self, blob: &ContentHash) -> Result<&[u8], EncodeError> {
        self.blobs
            .bytes(blob)
            .ok_or_else(|| EncodeError::MissingBlob(blob.clone()))
    }
}

/// 接上一块文字（`openai-chat.md` 第 4 条，施工 8-13 从 `openai_chat/messages.rs` 挪来，`openai-responses` 也照它拼）：前面有字、
/// 又不是以换行结尾的，先补一个换行。空的一块什么都不接。
pub(crate) fn join(into: &mut String, next: &str) {
    if next.is_empty() {
        return;
    }
    if !into.is_empty() && !into.ends_with('\n') {
        into.push('\n');
    }
    into.push_str(next);
}

/// 这是不是一个 PDF：文件只有 PDF 能照原样发。
pub(crate) fn is_pdf(file: &File) -> bool {
    file.media_type.as_str() == "application/pdf"
}

/// 图片块的名字：人附的有，`read` 读出来的没有。
fn name(image: &Image) -> Option<&str> {
    image.name.as_ref().map(|name| name.as_str())
}

/// 这份请求编码时要用哪些 blob（`openai-chat.md` 第 11 条）：user、tool 消息里的图片（模型能看图的），每一个文件（能读 PDF
/// 的发 PDF，别的要认是不是文本、要写有多大）。执行器照着先取出来。
pub(crate) fn blobs_needed(request: &Request, call: &Call) -> BTreeSet<ContentHash> {
    let mut needed = BTreeSet::new();
    for message in &request.messages {
        let blocks = match message {
            Message::User { blocks } | Message::Tool { blocks, .. } => blocks,
            Message::Assistant { .. } => continue,
        };
        for block in blocks {
            match block {
                Block::Image(image) if call.inputs.images => {
                    needed.insert(image.blob.clone());
                }
                Block::File(file) => {
                    needed.insert(file.blob.clone());
                }
                _ => {}
            }
        }
    }
    needed
}
