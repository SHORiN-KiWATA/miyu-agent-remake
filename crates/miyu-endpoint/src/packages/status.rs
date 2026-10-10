//! 软件包的状态、开关、后台页（施工 F-6 上，`docs/blueprint/package-pages.md`，设计 `30-插件框架.md` 第十三节）：`package.list`
//! 每一项多的 `status`、`enabled`、`page` 照这里算；程序不在的扩展、小程序当没装（配置项、功能都不算），也照这里认。

use std::path::PathBuf;

use miyu_config::package::{Manifest, PackageKind};
use miyu_store::packages::{Found, locate};

use crate::Core;
use crate::extensions::State;

/// 包的程序叫什么：扩展、界面照 `[command]`，小程序照 `[worker]`；内置包没有程序。
fn program(manifest: &Manifest) -> Option<&str> {
    match manifest.kind {
        PackageKind::Process | PackageKind::Ui => manifest
            .command
            .as_ref()
            .map(|command| command.program.as_str()),
        PackageKind::Worker => manifest
            .worker
            .as_ref()
            .map(|worker| worker.program.as_str()),
        PackageKind::Builtin => None,
    }
}

/// 主程序在哪：包的程序只找它旁边的（`packages.md`「转交」）。测试里是测试程序自己。
fn main_program() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("miyu"))
}

/// 要程序的包，程序不在 `miyu` 旁边。
pub(crate) fn program_missing(manifest: &Manifest) -> bool {
    program(manifest).is_some_and(|program| locate(program, &main_program()).is_none())
}

/// 当没装（`package-pages.md`「程序不在就当没装」）：核心拉起的那两种（扩展、小程序）程序不在。界面不算：它不是核心拉起的，
/// 连得上核心的界面自己就是程序在的证明（终端的会话 2026-10-10 提：开发时界面的程序不在 `miyu` 旁边）。
pub fn absent(manifest: &Manifest) -> bool {
    matches!(manifest.kind, PackageKind::Process | PackageKind::Worker) && program_missing(manifest)
}

/// 有没有后台页：清单写了 `[page]`，包目录里那个子目录下有 `index.html`。
pub(crate) fn page(found: &Found, manifest: &Manifest) -> bool {
    manifest
        .page
        .as_ref()
        .is_some_and(|dir| found.files_dir().join(dir).join("index.html").is_file())
}

/// `status`：`program_missing`、`off`、`running`、`starting`、`stopped`、`ready` 之一。`removed` 是出厂的包卸掉了。
pub(crate) fn status(core: &Core, id: &str, manifest: &Manifest, removed: bool) -> &'static str {
    if program_missing(manifest) {
        return "program_missing";
    }
    if removed {
        return "off";
    }
    if manifest.kind != PackageKind::Process {
        return "ready";
    }
    if !crate::extensions::switched_on(core, id, manifest) {
        return "off";
    }
    of_state(core.extensions.status(id).state)
}

/// 开着的扩展这时在哪一步，写成列表上的状态：退避中的等着再拉起，算启动中。
fn of_state(state: State) -> &'static str {
    match state {
        State::Off => "off",
        State::Starting { .. } | State::Waiting { .. } => "starting",
        State::Running { .. } => "running",
        State::Stopped(_) => "stopped",
    }
}

/// `enabled`：有开关的才有（`package-pages.md`「开关」）。扩展照它的开关；出厂的、不是必需的内置包照有没有卸掉；必需的、
/// 界面、小程序没有。`shipped` 是出厂的那一层。
pub(crate) fn enabled(
    core: &Core,
    id: &str,
    manifest: &Manifest,
    shipped: bool,
    removed: bool,
) -> Option<bool> {
    match manifest.kind {
        PackageKind::Process => {
            Some(!removed && crate::extensions::switched_on(core, id, manifest))
        }
        PackageKind::Builtin if shipped && !manifest.required => Some(!removed),
        PackageKind::Builtin | PackageKind::Ui | PackageKind::Worker => None,
    }
}

#[cfg(test)]
mod tests;
