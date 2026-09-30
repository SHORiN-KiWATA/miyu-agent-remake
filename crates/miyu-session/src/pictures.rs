//! 工具交回的图片（施工 4-13，`docs/blueprint/session/tools.md`「效果存成 blob」）：存成属主的 blob，换成内核的
//! 图片块，照先后接在内容块后面。

use miyu_kernel::block::{Block, Image};
use miyu_store::blob::Blobs;
use miyu_tool::Picture;

use crate::TARGET;

/// 存每一张图，交回图片块。有一张存不下来的（磁盘满了之类）交回空的，记一行运行日志：少了字节的图，以后每次
/// 请求都发不出去，这次调用照崩了算。碰磁盘，在阻塞线程里调。
pub(crate) fn store(blobs: &Blobs, pictures: Vec<Picture>) -> Option<Vec<Block>> {
    let mut blocks = Vec::with_capacity(pictures.len());
    for picture in pictures {
        match blobs.put(&picture.bytes) {
            Ok(blob) => blocks.push(Block::Image(Image {
                blob,
                // 工具读出来的图不带名字：那一次调用本来写着路径（施工 3-9 四补，`kernel/blocks.md` 第 14 条）。
                name: None,
                media_type: picture.media_type,
                width: picture.width,
                height: picture.height,
            })),
            Err(error) => {
                tracing::error!(target: TARGET, error = %error, "image not stored");
                return None;
            }
        }
    }
    Some(blocks)
}

#[cfg(test)]
mod tests;
