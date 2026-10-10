//! `miyu onebot web`（`onebot.md` 第一条「对外的样子」，施工 O-28 补）：打开网页软件里接入QQ 的后台页。
//!
//! 打开网页、登录、找后台页都是网页软件的事（`package-pages.md`「终端」第 2 条：`miyu web --package <编号>`），桥不另做一份：
//! 跑 `miyu` 旁边的主程序（找法照转交，`packages.md`「怎么走」第 5 条：`miyu onebot …` 是 `miyu` 转给旁边的 `miyu-onebot`），
//! 参数是 `web --package onebot`，标准输入输出照原样接着，等它退出，退出码照它的（照 `miyu web` 转给网页软件的写法）。不连核心、
//! 不读数据根：这些 `miyu web` 自己做。

use std::io::Write;
use std::path::Path;
use std::process::Command;

use crate::PACKAGE;
use crate::texts::Texts;

/// 没办成的退出码。
const FAILED: u8 = 1;

/// 跑 `miyu web --package onebot`，`miyu` 是主程序的路径（程序里是 `miyu-onebot` 旁边的那个），交回退出码：照它的；被信号
/// 停下的、退出码放不进一个字节的算 1。跑不了（不在、不让跑）的照 `texts` 在 `err` 上说一句（`web/no-miyu`），退出码 1。
pub fn web(miyu: &Path, texts: &Texts, err: &mut dyn Write) -> u8 {
    match Command::new(miyu)
        .args(["web", "--package", PACKAGE])
        .status()
    {
        Ok(status) => status
            .code()
            .and_then(|code| u8::try_from(code).ok())
            .unwrap_or(FAILED),
        Err(error) => {
            let said = texts.no_miyu(&miyu.display().to_string(), &error.to_string());
            if writeln!(err, "{said}").is_err() {
                // 标准错误关了：没有别处可说。
            }
            FAILED
        }
    }
}
