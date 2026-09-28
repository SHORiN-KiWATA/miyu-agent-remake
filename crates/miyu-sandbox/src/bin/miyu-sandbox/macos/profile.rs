//! 照规格写 Seatbelt 配置和参数（`docs/blueprint/sandbox/macos.md`「配置怎么写」）：只拼字，不碰系统，别的 Unix 上也
//! 编进测试。交进来的是换过的规格（`resolve.rs`）。
//!
//! Seatbelt 定先后的规矩（施工 5-7 在 CI 的 macOS 上量的）：同一件事，写得细的规则压过笼统的（`file-read-metadata`
//! 压过 `file-read*`，不管谁在前）；写得一样细的，后面的压过前面的。所以要互相压的放行和不许，写的是同一串名字，
//! 先后才管用：
//!
//! - 规格的每一条照路径的深浅排，深的在后，最深的那一条说了算；一样深的，藏起来的排在能写的后面（`sandbox.md`：
//!   几条重叠的时候越深的越算数，一样深的越严的算）。
//! - 删不掉、改不了名的目录排在最后。它写的 `file-write-unlink` 比 `file-write*` 细，放在哪都压得过放行的。
//!
//! 路径不写进配置的字里，写成参数（`(param "WRITE_0")`）：引号、反斜杠、中文都不用转义。

use std::path::{Component, Path, PathBuf};

use super::resolve::Resolved;

#[cfg(test)]
mod tests;

/// 配置的底子：不看规格，每条命令都一样（`base.sb`，蓝图里的样本）。
pub(crate) const BASE: &str = include_str!("base.sb");

/// 一份配置：交给系统的字，和字里 `(param "…")` 说的是哪条路径，照写进去的先后。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Profile {
    /// 配置的字。
    pub(crate) text: String,
    /// 参数：名字和它说的路径。
    pub(crate) params: Vec<(String, PathBuf)>,
}

/// 规格里的哪一样。先后就是一样深时的先后：越严的越后，写在后面的压过前面的。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    /// 能读能写。
    Write,
    /// 藏起来：读写都不行。
    Hidden,
}

impl Kind {
    /// 参数名字的开头。
    fn prefix(self) -> &'static str {
        match self {
            Kind::Write => "WRITE",
            Kind::Hidden => "HIDDEN",
        }
    }

    /// 这一样里的一条写成的规则，参数叫 `name`。放行和不许写的是同一串名字（见开头），和底子里整盘放开读的那一条
    /// 也一样：藏起来的压得过它，更深的能写的又压得过藏起来的。Unix 套接字照读写的规矩：能写的地方连得上，藏起来的
    /// 连不上。
    fn rules(self, name: &str) -> String {
        let socket = format!("(remote unix-socket (subpath (param \"{name}\")))");
        match self {
            Kind::Write => format!(
                "(allow file-read* file-read-metadata file-test-existence file-map-executable process-exec file-write* (subpath (param \"{name}\")))\n(allow network-outbound {socket})\n"
            ),
            Kind::Hidden => format!(
                "(deny file-read* file-read-metadata file-write* file-test-existence file-map-executable process-exec (literal (param \"{name}\")) (subpath (param \"{name}\")))\n(deny network-outbound {socket})\n"
            ),
        }
    }
}

/// 整份配置：底子，接着照规格生成的。
pub(crate) fn build(paths: &Resolved) -> Profile {
    let rules = rules(paths);
    Profile {
        text: format!("{BASE}{}", rules.text),
        params: rules.params,
    }
}

/// 照规格生成的那一段：接在底子后面（样本 `docs/designs/samples/sandbox/macos.sb` 就是它）。
pub(crate) fn rules(paths: &Resolved) -> Profile {
    let mut entries: Vec<(usize, Kind, &Path)> =
        [(Kind::Write, &paths.write), (Kind::Hidden, &paths.hidden)]
            .into_iter()
            .flat_map(|(kind, list)| {
                list.iter()
                    .map(move |path| (depth(path), kind, path.as_path()))
            })
            .collect();
    entries.sort_by(|a, b| {
        (a.0, a.1)
            .cmp(&(b.0, b.1))
            .then_with(|| bytes(a.2).cmp(bytes(b.2)))
    });
    let mut text = String::new();
    let mut params = Vec::new();
    let mut counts = [0_usize; 2];
    for (_, kind, path) in entries {
        let count = &mut counts[kind as usize];
        let name = format!("{}_{count}", kind.prefix());
        *count += 1;
        text.push_str(&kind.rules(&name));
        params.push((name, path.to_path_buf()));
    }
    for (index, path) in kept(paths).into_iter().enumerate() {
        let name = format!("KEEP_{index}");
        text.push_str(&format!(
            "(deny file-write-unlink (require-all (vnode-type DIRECTORY) (literal (param \"{name}\"))))\n"
        ));
        params.push((name, path.to_path_buf()));
    }
    Profile { text, params }
}

/// 删不掉、改不了名的目录：规格的每一条和它的每一层上级，凡是落在能写的那几片里的（能写的那一片自己也算），照
/// 路径的字节排。Seatbelt 照路径管：数据根的上级能改名，数据根就挪到了藏起来的那条路径外面；能写的那一片能删，
/// 删了换成指到别处的链接，下一条命令照它写规格，就放开了别处。
fn kept(paths: &Resolved) -> Vec<&Path> {
    let mut kept: Vec<&Path> = paths
        .write
        .iter()
        .chain(&paths.hidden)
        .flat_map(|path| path.ancestors())
        .filter(|place| {
            paths
                .write
                .iter()
                .any(|writable| place.starts_with(writable))
        })
        .collect();
    kept.sort_by(|a, b| bytes(a).cmp(bytes(b)));
    kept.dedup();
    kept
}

/// 路径有几段：根目录是 0，`/usr` 是 1。
fn depth(path: &Path) -> usize {
    path.components()
        .filter(|part| matches!(part, Component::Normal(_)))
        .count()
}

/// 路径的字节：一样深、一样的一样的，照它排。
fn bytes(path: &Path) -> &[u8] {
    path.as_os_str().as_encoded_bytes()
}
