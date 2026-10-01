//! 转述一张图的请求（施工 8-17，`docs/blueprint/kernel/request.md`「替它看的图」）：主对话的模型看不了图，内核把一张图交给
//! `models.vision` 转成字。单独一次辅助请求，不接主对话的前缀、不带 system 和工具面，一条 user：
//!
//! 1. 一块字：指令；人这一轮最近说的那一句有的，接 `question.txt` 和那一句的原话（不转义、不截）。换行都在两份文件里，
//!    拼的时候不加字。
//! 2. 这张图，去掉名字：转述只看画面，文件名主请求里另写。

use miyu_kernel::block::{Block, Image, Text};
use miyu_kernel::request::{Message, Request};

use crate::texts::Vision;

/// 转述 `image` 的请求；`said` 是人这一轮最近说的那一句，没有的是 `None`。
pub(crate) fn request(image: &Image, said: Option<&str>, vision: &Vision) -> Request {
    let mut text = vision.instruction.clone();
    if let Some(said) = said {
        text.push_str(&vision.question);
        text.push_str(said);
    }
    let picture = Image {
        name: None,
        ..image.clone()
    };
    Request {
        tools: Vec::new(),
        system: String::new(),
        messages: vec![Message::User {
            blocks: vec![Block::Text(Text { text }), Block::Image(picture)],
        }],
        stable: 0,
        continuation: false,
        described: Default::default(),
    }
}

#[cfg(test)]
mod tests;
