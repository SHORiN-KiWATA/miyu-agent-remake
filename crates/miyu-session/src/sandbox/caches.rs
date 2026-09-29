//! 工具链的缓存用沙盒自己的一份（`docs/blueprint/session/tools.md` 第 1a 条第 5 步，施工 5-4 下）：工作区这一级，cargo、
//! npm、pip、go 的缓存经它们自己的环境变量指到沙盒的缓存里；cargo 的全局配置带过去，发布用的令牌不带。
//!
//! 一起派的几件只读的、同一个属主的几个会话会同时来备：建目录、建链接撞上了，照结果对不对算，不当出错。

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

/// 沙盒的缓存：这个会话的属主的那一份在哪，你的 cargo 目录在哪。核心算出来，造会话、载入时交进来。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxCache {
    /// 属主的那一份：`<缓存目录>/sandbox/<属主>`。
    pub dir: PathBuf,
    /// 你的 cargo 目录：核心的环境里 `CARGO_HOME` 设了、不是空的照它，不然 `~/.cargo`；不知道的是空的。
    pub cargo_home: Option<PathBuf>,
}

/// 每个工具链的环境变量，和它在沙盒的缓存里的那一处。
const VARIABLES: [(&str, &[&str]); 5] = [
    ("CARGO_HOME", &["cargo"]),
    ("npm_config_cache", &["npm"]),
    ("PIP_CACHE_DIR", &["pip"]),
    ("GOMODCACHE", &["go", "mod"]),
    ("GOCACHE", &["go", "build"]),
];

impl SandboxCache {
    /// 备好这一份：没有就建（只给本人，查法和沙盒自己的临时目录一样），带上 cargo 的配置。交回要放能写的那一处（真实的
    /// 位置）和要设的环境变量。
    ///
    /// # Errors
    ///
    /// 建不成、有了却不是只给本人的，链接建不成。
    pub(super) fn prepare(
        &self,
        owner_of: &Path,
    ) -> io::Result<(PathBuf, Vec<(OsString, OsString)>)> {
        super::private_dir(&self.dir, owner_of)?;
        let dir = super::real(&self.dir);
        carry_cargo_config(&dir.join("cargo"), self.cargo_home.as_deref())?;
        let env = VARIABLES
            .iter()
            .map(|(name, parts)| {
                let mut place = dir.clone();
                place.extend(parts.iter());
                (OsString::from(name), place.into_os_string())
            })
            .collect();
        Ok((dir, env))
    }
}

/// 把你的 cargo 配置带过去：`<沙盒的 cargo>/config.toml` 指到 `<你的 cargo 目录>/config.toml`。你的没有的，不建，留着的
/// 旧的删掉：指到不存在处的配置，cargo 会报错。
fn carry_cargo_config(cargo: &Path, yours: Option<&Path>) -> io::Result<()> {
    let carried = cargo.join("config.toml");
    let source = yours
        .map(|home| home.join("config.toml"))
        .filter(|source| source.is_file());
    match source {
        Some(source) => carry(&source, cargo, &carried),
        None => match std::fs::remove_file(&carried) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        },
    }
}

/// Unix：建指到 `source` 的链接，指错了的换掉。
#[cfg(unix)]
fn carry(source: &Path, cargo: &Path, link: &Path) -> io::Result<()> {
    let points_right = || std::fs::read_link(link).is_ok_and(|to| to == source);
    if points_right() {
        return Ok(());
    }
    match std::fs::remove_file(link) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    std::fs::create_dir_all(cargo)?;
    match std::os::unix::fs::symlink(source, link) {
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists && points_right() => Ok(()),
        other => other,
    }
}

/// Windows：建符号链接要开发者模式，照原样拷一份，内容变了再拷。
#[cfg(windows)]
fn carry(source: &Path, cargo: &Path, copy: &Path) -> io::Result<()> {
    let fresh = std::fs::read(source)?;
    if std::fs::read(copy).is_ok_and(|old| old == fresh) {
        return Ok(());
    }
    std::fs::create_dir_all(cargo)?;
    std::fs::write(copy, fresh)
}
