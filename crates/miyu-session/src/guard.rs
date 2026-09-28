//! 权限策略（`11-权限与沙盒.md` 第二节「第一版的权限策略怎么判」，施工 4-3 下）：执行前那条链的默认实现，
//! 模块编号 `permissions`。工具报出这次调用要碰的路径，换成真实的位置、查边界表，每一条照实际生效的那一级判，
//! 合起来照最严的：有一条拒绝就拒绝，有一条要问人就问人。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};

use miyu_fs::{Boundary, Places, ResolveError, Zone, resolve, resolve_itself};
use miyu_kernel::event::{Level, Permission};
use miyu_kernel::id::ModuleId;
use miyu_kernel::raw::RawJson;
use miyu_kernel::session::Verdict;
use miyu_kernel::tool::{Access, Worded};
use miyu_policy::GuardTexts;
use miyu_tool::{Call, Catalog, Stop, Target};

/// 权限策略：一个会话一份。
pub(crate) struct Guard {
    catalog: Catalog,
    data_root: PathBuf,
    home: Option<PathBuf>,
    texts: GuardTexts,
}

/// 实际生效的那一级。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Effective {
    /// 完全放开。
    Full,
    /// 工作区。
    Workspace,
    /// 只读：开着只读开关，或者常用的那一级不认识（按最严的算）。
    ReadOnly,
}

/// 一条路径判下来是什么。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mark {
    /// 放行。
    Allow,
    /// 要问人。
    Ask,
    /// 拒绝：碰到了数据根。
    Forbidden,
    /// 拒绝：只读的时候要写。
    ReadOnly,
}

/// 要问人的一条路径：真实的位置、是不是写、在哪一片。
struct Asked {
    real: PathBuf,
    write: bool,
    zone: Zone,
}

impl Guard {
    /// 照目录 `catalog` 找工具，数据根是 `data_root`，家目录是 `home`，拒绝时的话是 `texts`。
    pub(crate) fn new(
        catalog: Catalog,
        data_root: PathBuf,
        home: Option<PathBuf>,
        texts: GuardTexts,
    ) -> Guard {
        Guard {
            catalog,
            data_root,
            home,
            texts,
        }
    }

    /// 判一次调用：工具名 `name`，修正过的参数 `args`，这一轮的工作目录 `cwd`，实际生效的级别 `permission`。
    pub(crate) fn judge(
        &self,
        name: &str,
        args: String,
        cwd: String,
        permission: &Permission,
    ) -> Verdict {
        // 目录里没有的：放行，执行时报现在用不了（施工 4-2）。
        let Some(tool) = self.catalog.get(name) else {
            return Verdict::Allow;
        };
        let level = effective(permission);
        let access = tool.spec().access.clone();
        // 报要碰的路径只看参数，用不着她看过的。
        let targets = tool.targets(&Call {
            args,
            cwd: cwd.clone(),
            home: self.home.clone(),
            data_root: Some(self.data_root.clone()),
            seen: Arc::default(),
            stop: Stop::default(),
        });
        if targets.is_empty() {
            return untargeted(level, name, access);
        }
        // 工作目录本身也换成真实的位置：头报来的可能是 `~`。
        let cwd = resolve(Path::new(&cwd), self.home.as_deref(), &cwd)
            .unwrap_or_else(|_| PathBuf::from(&cwd));
        let places = Places::here(cwd.clone(), self.data_root.clone(), self.home.as_deref());
        let boundary = Boundary::new(&places);
        let mut asked = Vec::new();
        for target in targets {
            let real = match self.real(&cwd, &target) {
                Ok(real) => real,
                Err(error) => {
                    return deny(self.texts.unresolvable(&target.path, &error.to_string()));
                }
            };
            let zone = boundary.zone(&real);
            match mark(level, zone, target.write) {
                Mark::Allow => {}
                Mark::Ask => asked.push(Asked {
                    real,
                    write: target.write,
                    zone,
                }),
                Mark::Forbidden => return deny(self.texts.forbidden(&target.path)),
                Mark::ReadOnly => return deny(self.texts.read_only()),
            }
        }
        if asked.is_empty() {
            Verdict::Allow
        } else {
            ask(name, access, &asked)
        }
    }
}

impl Guard {
    /// 要碰的这一条换成真实的位置。碰的是这一条本身的（`trash`，施工 4-9 再补二）：最后一段不跟链接，和工具碰的是
    /// 同一个；没有名字可碰的（`.`、`..`、`~`），照整条换。
    fn real(&self, cwd: &Path, target: &Target) -> Result<PathBuf, ResolveError> {
        let home = self.home.as_deref();
        if target.itself
            && let Some(real) = resolve_itself(cwd, home, &target.path)?
        {
            return Ok(real);
        }
        resolve(cwd, home, &target.path)
    }
}

/// 实际生效的那一级。
fn effective(permission: &Permission) -> Effective {
    if permission.read_only {
        return Effective::ReadOnly;
    }
    match permission.level {
        Level::Full => Effective::Full,
        Level::Workspace => Effective::Workspace,
        Level::Other(_) => Effective::ReadOnly,
    }
}

/// 一条路径照级别判（11 第二节的判法表）。
fn mark(level: Effective, zone: Zone, write: bool) -> Mark {
    match (zone, level, write) {
        (Zone::Forbidden, _, _) => Mark::Forbidden,
        (_, Effective::Full, _) | (Zone::Writable | Zone::Readable, _, false) => Mark::Allow,
        (_, Effective::ReadOnly, true) => Mark::ReadOnly,
        (Zone::Writable, Effective::Workspace, true) => Mark::Allow,
        (Zone::Readable | Zone::Outside, Effective::Workspace, true)
        | (Zone::Outside, Effective::Workspace | Effective::ReadOnly, false) => Mark::Ask,
    }
}

/// 不报路径的调用：执行命令照级别（M5 之前还没有沙盒，工作区这一级放行，只读时问人），读写放行（查不到路径的
/// 执行时自己报错），联网、对外发消息这些 M4 还没有的，除了完全放开都问人。
fn untargeted(level: Effective, name: &str, access: Access) -> Verdict {
    let fine = matches!(
        (&access, level),
        (_, Effective::Full)
            | (Access::Read | Access::Write, _)
            | (Access::Execute, Effective::Workspace)
    );
    if fine {
        return Verdict::Allow;
    }
    Verdict::Ask {
        module: module(),
        access,
        rule: None,
        detail: Some(raw(&json!({ "tool": name }))),
    }
}

/// 要问人：提的放行规则列出越界的目录，给头看的说明列出每一条。
fn ask(name: &str, access: Access, asked: &[Asked]) -> Verdict {
    let dirs = |write: bool| -> Vec<String> {
        let mut dirs: Vec<String> = asked
            .iter()
            .filter(|asked| asked.write == write)
            .map(|asked| text(&directory(&asked.real)))
            .collect();
        dirs.sort();
        dirs.dedup();
        dirs
    };
    let mut rule = json!({ "tool": name });
    for (key, write) in [("read", false), ("write", true)] {
        let listed = dirs(write);
        if !listed.is_empty() {
            rule[key] = json!(listed);
        }
    }
    let paths: Vec<Value> = asked
        .iter()
        .map(|asked| {
            json!({
                "path": text(&asked.real),
                "write": asked.write,
                "zone": if asked.zone == Zone::Outside { "outside" } else { "read_only" },
            })
        })
        .collect();
    Verdict::Ask {
        module: module(),
        access,
        rule: Some(raw(&rule)),
        detail: Some(raw(&json!({ "tool": name, "paths": paths }))),
    }
}

/// 拒绝，写给她这一句，连同给人看的说法。
fn deny(worded: Worded) -> Verdict {
    Verdict::Deny {
        module: module(),
        text: worded.text,
        human: worded.said,
    }
}

/// 权限策略的模块编号。
///
/// # Panics
///
/// 实际不会：`permissions` 合模块编号的写法。
fn module() -> ModuleId {
    ModuleId::parse("permissions").expect("permissions 合模块编号的写法")
}

/// 放行的范围：是目录的就是它自己，别的是它所在的目录。
fn directory(real: &Path) -> PathBuf {
    if real.is_dir() {
        real.to_path_buf()
    } else {
        real.parent()
            .map_or_else(|| real.to_path_buf(), Path::to_path_buf)
    }
}

/// 路径写成字。
fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// 自己拼的 JSON，写成原样的 JSON。
///
/// # Panics
///
/// 实际不会：自己拼的 JSON 一定读得回来。
fn raw(value: &Value) -> RawJson {
    serde_json::from_str(&value.to_string()).expect("自己拼的 JSON 读得回来")
}

#[cfg(test)]
mod tests;
