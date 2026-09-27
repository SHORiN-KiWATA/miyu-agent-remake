//! Windows 的命名管道只对本人开放（`docs/designs/04-核心协议.md` 第二节「各平台的坑」，施工 3-8 补）。
//!
//! 命名管道默认的安全描述符允许 Everyone 读，要显式设成只允许当前用户；头连上以后，还要核对管道另一头
//! 的进程是自己的。这几样要调 Windows 的安全接口，整个仓库只有这个 crate 放开了 `unsafe`（2026-09-27
//! 项目主人定：单开一个小 crate），而且只装非用它不可的三样：
//!
//! - `create`：建管道的一个实例，只对当前用户开放，可以声明是第一个实例；
//! - `current_user`：当前用户的 SID；
//! - `server_user`：管道另一头（服务端）进程的用户 SID。
//!
//! 等连接、管道忙了再连这些用不着 `unsafe` 的，在 `miyu-ipc` 里。别的平台上这个 crate 只有
//! [`owner_only`]。

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::{create, current_user, server_user};

/// 只允许 `sid` 这个用户的安全描述符，写成 SDDL：`D:` 是访问控制列表，`P` 是受保护、不继承上一层的；
/// 只有一条：允许（`A`）全部权限（`GA`）给他。
pub fn owner_only(sid: &str) -> String {
    format!("D:P(A;;GA;;;{sid})")
}

#[cfg(test)]
mod tests;
