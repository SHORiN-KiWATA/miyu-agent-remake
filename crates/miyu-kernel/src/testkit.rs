//! 测试用的执行器替身（`docs/designs/02-内核.md` 第四节「执行器怎么回动作」）：照剧本回模型、
//! 回工具、回执行前的链、回挂接点，替会话落盘，把真会话一整轮一整轮地跑完。
//!
//! 只在 `testkit` 开关打开时编进去，内核自己的测试里总是有（`01-架构.md` 第九节）。它不做 I/O：
//! 「磁盘」是一串内存里的事件，「模型」「工具」照剧本回。
//!
//! - [`Line`]：模型的一次回复；
//! - [`Play`]：一次工具调用怎么回；
//! - [`Stage`]：替身本身，人的每个动作以后一直跑到没事可做；子会话、执行器交来的回报也照样跑（施工 7-2，`jobs.rs`）；
//! - [`restored`]：替身改回的一步。

mod compact;
mod disk;
mod jobs;
mod limits;
mod opening;
mod respond;
mod script;
mod stage;

pub use respond::model;
pub use script::{Line, Play};
pub use stage::Stage;

use crate::event::{RestoreAction, Restored};
use crate::session::Step;

/// 替身改回的一步：照做成了。移进回收站的，和真的执行器一样带着回收站里的位置（施工 4-9 再补一：内核对照交回的
/// 结局，移进回收站成了、没带位置的算没做成）。
pub fn restored(step: &Step) -> Restored {
    let mut done = step.restored();
    if done.action == RestoreAction::Trash {
        done.trash = Some(format!("trash/{}-{}", step.result, step.effect));
    }
    done
}
