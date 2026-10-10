//! 软件后台（施工 F-6 中，`docs/blueprint/package-pages.md`，设计 `30-插件框架.md` 第十三节）：带自己页面的软件，页面的文件
//! 经 `package.file` 拿（头不读核心的存储，远程的头也照样能用），页面要它的程序做的事经 `package.call` 转过去：程序先用
//! `package.methods` 登记方法，核心反向发 `method.call`。页面能做的只有这几样，别的本事由它的程序用自己批过的能力做。

mod calls;
mod file;

pub(crate) use calls::{Methods, call, register};
pub(crate) use file::read;
