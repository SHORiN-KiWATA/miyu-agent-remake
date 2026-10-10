//! Landlock（`docs/blueprint/sandbox/linux.md`）：建规则集、照规格加规则、收紧自己。
//!
//! 第 2 版的全部文件操作必须管得住：第 2 版起才管得了跨目录的改名、链接，更老的一律不许，很多工具会坏。更新的
//! 版本多出来的（截断、设备的 ioctl、连路径上的 Unix 套接字）能管就管，这台内核没有的就少管那几样；沙盒外建的抽象
//! 套接字（第 6 版起）也能管就管。连套接字要管，是因为 D-Bus、Docker 这类系统服务能替命令在沙盒外读写（2026-09-29
//! 项目主人定：沙盒不管网络，但这类系统服务照样挡）。

use std::io;
use std::path::{Path, PathBuf};

use landlock::{
    ABI, Access, AccessFs, BitFlags, CompatLevel, Compatible, PathBeneath, PathFd, PathFdError,
    Ruleset, RulesetAttr, RulesetCreated, RulesetCreatedAttr, RulesetError, RulesetStatus, Scope,
};

use miyu_sandbox::Spec;

use super::reads::plan;

/// 至少要管得住的那一版。
const REQUIRED: ABI = ABI::V2;

/// 这个库认得的最新一版：能管就管到这一版。
const LATEST: ABI = ABI::V9;

/// 内核的 Landlock 够不够用：建一个要求第 2 版文件操作的规则集，建得成就够。只建不收紧，探测的进程不受影响。
pub(super) fn available() -> Result<(), String> {
    ruleset().map(drop)
}

/// 照规格收紧自己：根目录往下都能列目录、执行；读照 [`plan`] 一级级放，绕开藏起来的；`write` 和 `/dev/null` 放行
/// 全部。写了 `read` 的（施工 5-12 下）：只有它列的能读、列目录、执行，根目录不再整个放，`hidden` 不另算。规格里的路径不在
/// 的跳过。
pub(super) fn confine(spec: &Spec) -> Result<(), String> {
    let mut created = ruleset()?;
    match &spec.read {
        None => {
            created = add(
                created,
                Path::new("/"),
                AccessFs::ReadDir | AccessFs::Execute,
            )?;
            for path in plan(&spec.hidden)? {
                created = add_readable(created, &path)?;
            }
        }
        Some(read) => {
            for path in read {
                created = add_readable(created, path)?;
            }
        }
    }
    let null = PathBuf::from("/dev/null");
    for path in spec.write.iter().chain([&null]) {
        created = add(created, path, AccessFs::from_all(LATEST))?;
    }
    let status = created
        .restrict_self()
        .map_err(|error| format!("landlock: {error}"))?;
    match status.ruleset {
        RulesetStatus::NotEnforced => Err("landlock is not available: not enforced".to_string()),
        _ => Ok(()),
    }
}

/// 建规则集：第 2 版的文件操作必须管得住，更新的能管就管，抽象套接字能管就管；收紧时设 `no_new_privs`。
fn ruleset() -> Result<RulesetCreated, String> {
    let unavailable = |error: RulesetError| format!("landlock is not available: {error}");
    Ok(Ruleset::default()
        .set_compatibility(CompatLevel::HardRequirement)
        .handle_access(AccessFs::from_all(REQUIRED))
        .and_then(|ruleset| {
            ruleset
                .set_compatibility(CompatLevel::BestEffort)
                .handle_access(AccessFs::from_all(LATEST))
        })
        .and_then(|ruleset| ruleset.scope(Scope::AbstractUnixSocket))
        .and_then(Ruleset::create)
        .map_err(unavailable)?
        .no_new_privs(true))
}

/// 给 `path` 加一条放行读的规则；和 [`add`] 一样，只多一样：打开了却查不了它是什么的（断开的 FUSE 挂载，例如
/// 守护进程没了的 `~/.gvfs`，`fstat` 报「Transport endpoint is not connected」）跳过。它本来就读不了，跳过等于
/// 在沙盒里也读不了，不放宽什么；不跳过的话，[`plan`] 一级级列到它，整个沙盒就起不来、什么命令都跑不了（施工
/// 5-4 补，2026-10-01 项目主人的机器上撞见）。写的规则不这样：写要放行的路径坏了，照旧报错。
fn add_readable(created: RulesetCreated, path: &Path) -> Result<RulesetCreated, String> {
    match PathFd::new(path) {
        Ok(fd) if !describable(&fd) => Ok(created),
        _ => add(created, path, AccessFs::from_read(LATEST)),
    }
}

/// 打开了的路径查得了是什么（文件还是目录）：库加规则时要用 `fstat` 查它，查不了的整个规则集都加不上。
fn describable(fd: &PathFd) -> bool {
    use std::os::fd::AsFd;
    fd.as_fd()
        .try_clone_to_owned()
        .map(std::fs::File::from)
        .and_then(|file| file.metadata())
        .is_ok()
}

/// 给 `path` 加一条放行 `access` 的规则。不在的跳过；是文件的，只放行对文件有意义的那几样（库照最宽松的兼容级别
/// 自己去掉目录才有的）。
fn add(
    created: RulesetCreated,
    path: &Path,
    access: BitFlags<AccessFs>,
) -> Result<RulesetCreated, String> {
    let fd = match PathFd::new(path) {
        Ok(fd) => fd,
        Err(PathFdError::OpenCall { source, .. }) if source.kind() == io::ErrorKind::NotFound => {
            return Ok(created);
        }
        Err(PathFdError::OpenCall { source, .. }) => {
            return Err(format!("cannot open {}: {source}", path.display()));
        }
        Err(error) => return Err(format!("cannot open {}: {error}", path.display())),
    };
    created
        .add_rule(PathBeneath::new(fd, access))
        .map_err(|error| format!("landlock: {error}"))
}
