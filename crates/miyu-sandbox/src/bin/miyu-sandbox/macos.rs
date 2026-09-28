//! macOS 上的收紧（`docs/blueprint/sandbox/macos.md`，施工 5-7）：把规格写成一份 Seatbelt 配置，装到自己身上，再换成
//! 命令。配置只管读写，网络不管；能替命令到别处读写的系统服务照样挡（设计 `11-权限与沙盒.md` 第六节、A8）。
//!
//! - [`resolve`]：规格里的路径换成真实的位置，Seatbelt 照真实的位置比；
//! - [`profile`]：照规格写配置和参数，只拼字；
//! - `seatbelt`：调系统的接口装上，整个 crate 只有它用 `unsafe`。
//!
//! 前两块不碰 macOS 的接口，别的 Unix 上跑测试时也编进去；装上、探测、换成命令只在 macOS 上编。

mod profile;
mod resolve;
#[cfg(target_os = "macos")]
mod seatbelt;

#[cfg(target_os = "macos")]
use std::ffi::{OsStr, OsString};
#[cfg(target_os = "macos")]
use std::process::ExitCode;

#[cfg(target_os = "macos")]
use miyu_sandbox::Spec;

/// 探测时用的规格：每一种规则都用上（`probe.json`，蓝图里的样本）。
#[cfg(target_os = "macos")]
const PROBE: &str = include_str!("macos/probe.json");

/// 这台机器上能用上的收紧手段：照 `probe.json` 写一份配置、装到自己身上，装上了是 `seatbelt`。
///
/// 装上就收不回来：只有 `probe` 调它，印完那一行就退出。装不上的（例如 Miyu 自己跑在一个不许再装沙盒的沙盒里）
/// 报空的，原因不报，要看原因就手动跑一次 `run`。
#[cfg(target_os = "macos")]
pub(crate) fn mechanisms() -> Vec<String> {
    let confined = Spec::from_json(PROBE)
        .map_err(|error| error.to_string())
        .and_then(|spec| confine(&spec));
    match confined {
        Ok(()) => vec!["seatbelt".to_string()],
        Err(_) => Vec::new(),
    }
}

/// 照规格 `spec` 收紧，再换成那条命令。收不住的说原话，退出 125，不跑命令。
#[cfg(target_os = "macos")]
pub(crate) fn run(spec: &Spec, program: &OsStr, args: &[OsString]) -> ExitCode {
    if let Err(why) = confine(spec) {
        return crate::fail(&format!("cannot confine: {why}"));
    }
    crate::unix::exec(program, args)
}

/// 换成真实的位置、写配置、装上。
#[cfg(target_os = "macos")]
fn confine(spec: &Spec) -> Result<(), String> {
    let profile = profile::build(&resolve::resolve(spec)?);
    seatbelt::apply(&profile.text, &profile.params)
}
