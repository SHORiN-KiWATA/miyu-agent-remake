//! 边界表（`11-权限与沙盒.md` 第四节，施工 4-3 上）：一个真实的位置落在哪一片。
//!
//! 几片重叠时照这个先后，先对上的算：工作区（里面会在沙盒外被执行的那几样只能读）、数据根、临时目录、
//! 系统和工具链目录、边界以外。

use std::path::{Component, Path, PathBuf};

/// 一个真实的位置落在哪一片。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    /// 能读能写：工作区、临时目录。
    Writable,
    /// 只能读：系统目录、工具链目录，工作区里的 `.git/hooks`、`.git/config`。
    Readable,
    /// 谁都不能碰：Miyu 的数据根。
    Forbidden,
    /// 边界以外：要碰就得问人。
    Outside,
}

/// 造边界表要的几个地方。都是原样的路径，[`Boundary::new`] 再换成真实的位置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Places {
    /// 这一轮的工作区。给之前先照 `11-权限与沙盒.md` 第四节挑过：太宽的已经退回了账号的工作区。
    pub workspace: PathBuf,
    /// Miyu 的数据根。
    pub data_root: PathBuf,
    /// 临时目录。
    pub temp: PathBuf,
    /// 只能读的：系统目录、工具链目录。
    pub readable: Vec<PathBuf>,
}

/// 系统目录的第一版清单（11 第四节）。
#[cfg(target_os = "linux")]
const SYSTEM: &[&str] = &[
    "/usr", "/bin", "/sbin", "/lib", "/lib32", "/lib64", "/etc", "/opt",
];

/// 系统目录的第一版清单（11 第四节）。`/etc` 的真实位置在 `/private/etc`，换成真实的位置时自然就对上了。
#[cfg(target_os = "macos")]
const SYSTEM: &[&str] = &[
    "/usr",
    "/bin",
    "/sbin",
    "/System",
    "/Library",
    "/Applications",
    "/opt",
    "/etc",
];

/// 别的 Unix：还没有清单，系统目录一个都不开，要读就问人。
#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
const SYSTEM: &[&str] = &[];

/// 家目录下的工具链目录（11 第四节）。
const TOOLCHAINS: &[&str] = &[".cargo", ".rustup", ".npm", ".gitconfig"];

impl Places {
    /// 照这台机器：临时目录、系统目录、家目录下的工具链目录，`CARGO_HOME`、`RUSTUP_HOME` 指到的地方。
    /// 家目录读不出来的，工具链目录只算环境变量指到的。
    pub fn here(workspace: PathBuf, data_root: PathBuf, home: Option<&Path>) -> Places {
        let mut readable = system_dirs();
        if let Some(home) = home {
            readable.extend(TOOLCHAINS.iter().map(|name| home.join(name)));
        }
        for name in ["CARGO_HOME", "RUSTUP_HOME"] {
            if let Some(dir) = std::env::var_os(name).filter(|dir| !dir.is_empty()) {
                readable.push(PathBuf::from(dir));
            }
        }
        Places {
            workspace,
            data_root,
            temp: std::env::temp_dir(),
            readable,
        }
    }
}

/// 这台机器的系统目录。
#[cfg(unix)]
fn system_dirs() -> Vec<PathBuf> {
    SYSTEM.iter().map(PathBuf::from).collect()
}

/// 这台机器的系统目录：`%SystemRoot%`、`%ProgramFiles%`、`%ProgramFiles(x86)%`，没设的不算。
#[cfg(windows)]
fn system_dirs() -> Vec<PathBuf> {
    ["SystemRoot", "ProgramFiles", "ProgramFiles(x86)"]
        .iter()
        .filter_map(std::env::var_os)
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .collect()
}

/// 边界表：每一片都换成了真实的位置；不存在的那一片不算。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Boundary {
    workspace: Option<PathBuf>,
    data_root: Option<PathBuf>,
    temp: Option<PathBuf>,
    readable: Vec<PathBuf>,
}

impl Boundary {
    /// 照 `places` 造：每一片都换成真实的位置，不存在的（例如这台机器上没有 `/lib32`）不算。
    pub fn new(places: &Places) -> Boundary {
        Boundary {
            workspace: real(&places.workspace),
            data_root: real(&places.data_root),
            temp: real(&places.temp),
            readable: places.readable.iter().filter_map(|dir| real(dir)).collect(),
        }
    }

    /// 真实的位置 `path` 落在哪一片：先经 [`crate::resolve()`] 换过再交进来。
    pub fn zone(&self, path: &Path) -> Zone {
        if let Some(workspace) = &self.workspace
            && let Ok(inside) = path.strip_prefix(workspace)
        {
            return if runs_outside(inside) {
                Zone::Readable
            } else {
                Zone::Writable
            };
        }
        if self
            .data_root
            .as_ref()
            .is_some_and(|root| within(path, root))
        {
            return Zone::Forbidden;
        }
        if self
            .temp
            .as_ref()
            .is_some_and(|temp| path.starts_with(temp))
        {
            return Zone::Writable;
        }
        if self.readable.iter().any(|dir| path.starts_with(dir)) {
            return Zone::Readable;
        }
        Zone::Outside
    }
}

/// 换成真实的位置；不存在、读不了的不算。
fn real(path: &Path) -> Option<PathBuf> {
    std::fs::canonicalize(path).ok()
}

/// 工作区里的这一段，是不是会在沙盒外被执行的：哪一层有个 `.git`，下一层是 `hooks` 或者 `config`。
/// 大小写不分的文件系统上，`.GIT/HOOKS` 也是同一处，照不分大小写比。
fn runs_outside(inside: &Path) -> bool {
    let names: Vec<String> = inside
        .components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(fold(&name.to_string_lossy())),
            _ => None,
        })
        .collect();
    names
        .windows(2)
        .any(|pair| pair[0] == ".git" && (pair[1] == "hooks" || pair[1] == "config"))
}

/// 真实的位置 `path` 在不在目录 `root` 里（`root` 自己也算）：大小写不分的平台上不分大小写，免得换个大小写就
/// 绕过去。两个都要先换成真实的位置。往下走目录的工具照它跳过 Miyu 的数据根（施工 4-4 下）。
pub fn within(path: &Path, root: &Path) -> bool {
    if !CASE_INSENSITIVE {
        return path.starts_with(root);
    }
    let path: Vec<String> = path
        .components()
        .map(|c| fold(&c.as_os_str().to_string_lossy()))
        .collect();
    let root: Vec<String> = root
        .components()
        .map(|c| fold(&c.as_os_str().to_string_lossy()))
        .collect();
    path.starts_with(&root)
}

/// 这个平台的文件系统通常不分大小写：macOS、Windows。
const CASE_INSENSITIVE: bool = cfg!(any(target_os = "macos", windows));

/// 比较用的写法：大小写不分的平台上换成小写，别的照原样。
fn fold(name: &str) -> String {
    if CASE_INSENSITIVE {
        name.to_lowercase()
    } else {
        name.to_string()
    }
}
