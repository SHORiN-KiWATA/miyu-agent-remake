//! Landlock（`docs/blueprint/sandbox/linux.md`）：建规则集、照规格加规则、收紧自己。
//!
//! 第 2 版的全部文件操作必须管得住：第 2 版起才管得了跨目录的改名、链接，更老的一律不许，很多工具会坏。更新的
//! 版本多出来的文件操作（截断、设备的 ioctl、连路径上的 Unix 套接字）能管就管，这台内核没有的就少管那几样。

use std::io;
use std::path::Path;

use landlock::{
    ABI, Access, AccessFs, BitFlags, CompatLevel, Compatible, PathBeneath, PathFd, PathFdError,
    Ruleset, RulesetAttr, RulesetCreated, RulesetCreatedAttr, RulesetError, RulesetStatus,
};

use miyu_sandbox::Spec;

/// 至少要管得住的那一版。
const REQUIRED: ABI = ABI::V2;

/// 这个库认得的最新一版：能管就管到这一版。
const LATEST: ABI = ABI::V9;

/// 内核的 Landlock 够不够用：建一个要求第 2 版文件操作的规则集，建得成就够。只建不收紧，探测的进程不受影响。
pub(super) fn available() -> Result<(), String> {
    ruleset().map(drop)
}

/// 照规格收紧自己：`read`、`readonly` 放行读，`write` 放行全部。规格里的路径不在的跳过。
pub(super) fn confine(spec: &Spec) -> Result<(), String> {
    let mut created = ruleset()?;
    let read = AccessFs::from_read(LATEST);
    for path in spec.read.iter().chain(&spec.readonly) {
        created = add(created, path, read)?;
    }
    for path in &spec.write {
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

/// 建规则集：第 2 版的文件操作必须管得住，更新的能管就管；收紧时设 `no_new_privs`。
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
        .and_then(Ruleset::create)
        .map_err(unavailable)?
        .no_new_privs(true))
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
