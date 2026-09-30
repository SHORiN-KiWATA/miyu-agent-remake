//! `session.send` 的 `from`（施工 7-10，`docs/blueprint/protocol.md` 的 `session.send` 第 6 条，`agents.md` 第十一条第 4
//! 条）：别的 harness 报的自己的名字，这一句记成它说的，`by` 是 `harness`。
//!
//! 名字是对方自己报的，不核对，只管写法：照短名字的规矩（`kernel/ids.md`），先去掉控制字符，再截到 128 字节以内，不截断
//! 一个字；剩下是空的，参数不对。给模型看之前的转义归组装（`kernel/request.md`「别的 harness 发来的话」）。

use miyu_kernel::id::HarnessName;
use miyu_kernel::origin::{By, Harness};

use crate::refusal::Refusal;

/// 名字最多几个字节：短名字的上限（`kernel/ids.md`「用字符串写的」第 1 条）。
const BYTES: usize = 128;

/// 别的 harness 报的名字 `name` 收成这一句的 `by`。
///
/// # Errors
///
/// 去掉控制字符以后是空的：参数不对。
pub(crate) fn harness(name: &str) -> Result<By, Refusal> {
    let mut kept = String::new();
    for c in name.chars().filter(|c| !c.is_control()) {
        if kept.len() + c.len_utf8() > BYTES {
            break;
        }
        kept.push(c);
    }
    let name = HarnessName::parse(&kept).map_err(|_| Refusal::BAD_PARAMS)?;
    Ok(By::Harness(Harness { name }))
}

#[cfg(test)]
mod tests;
