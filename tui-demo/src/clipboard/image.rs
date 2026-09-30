//! 读系统剪贴板里的图（`Ctrl+V`，蓝图 `tui.md`「输入框」第 12 条）：有图的存一份到给的目录（照内容起名，同一张不存
//! 两份），交回文件，由输入框收成附件。没有图的交回 `None`，照旧读字。
//!
//! Linux 有 Wayland 的先问 `wl-paste --list-types` 有没有图，再问 `xclip` 的 `TARGETS`；macOS 用 `osascript` 取
//! `«class PNGf»`。每个命令最多等一会儿，卡住的放弃（`read.rs`）。

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use super::read::{WAIT, run_bytes};

/// 认的几种图（核心只收这几种），先后是挑的先后：媒体类型和存成的扩展名。
const KINDS: [(&str, &str); 4] = [
    ("image/png", "png"),
    ("image/jpeg", "jpg"),
    ("image/gif", "gif"),
    ("image/webp", "webp"),
];

/// 剪贴板里有图的，存进 `dir`，交回文件；没有图、读不到、存不下的交回 `None`。
pub fn read_image(dir: &Path) -> Option<PathBuf> {
    let (bytes, ext) = grab()?;
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    let file = dir.join(format!("{:016x}.{ext}", hasher.finish()));
    if !file.is_file() {
        fs::create_dir_all(dir).ok()?;
        fs::write(&file, &bytes).ok()?;
    }
    Some(file)
}

/// 照这台机器的剪贴板命令取图：字节和扩展名。
fn grab() -> Option<(Vec<u8>, &'static str)> {
    if cfg!(target_os = "macos") {
        let out = run_bytes("osascript", &["-e", "the clipboard as «class PNGf»"], WAIT)?;
        return mac_png(&String::from_utf8_lossy(&out)).map(|png| (png, "png"));
    }
    if cfg!(windows) {
        return None;
    }
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    let from_wayland = || {
        let types = run_bytes("wl-paste", &["--list-types"], WAIT)?;
        let (mime, ext) = pick(&String::from_utf8_lossy(&types))?;
        Some((run_bytes("wl-paste", &["--type", mime], WAIT)?, ext))
    };
    let from_x11 = || {
        let clip = ["-selection", "clipboard", "-t"];
        let types = run_bytes("xclip", &[clip[0], clip[1], clip[2], "TARGETS", "-o"], WAIT)?;
        let (mime, ext) = pick(&String::from_utf8_lossy(&types))?;
        Some((
            run_bytes("xclip", &[clip[0], clip[1], clip[2], mime, "-o"], WAIT)?,
            ext,
        ))
    };
    let got = if wayland { from_wayland() } else { None };
    got.or_else(from_x11).filter(|(bytes, _)| !bytes.is_empty())
}

/// 剪贴板说它有的几种里挑一种认得的图：一行一种。
fn pick(types: &str) -> Option<(&'static str, &'static str)> {
    let have: Vec<&str> = types.lines().map(str::trim).collect();
    KINDS.into_iter().find(|(mime, _)| have.contains(mime))
}

/// `osascript` 取出来的是 `«data PNGf89504E47…»`：取出十六进制那一截换成字节；不是图的是 `None`。
fn mac_png(out: &str) -> Option<Vec<u8>> {
    let hex = out.trim().strip_prefix("«data PNGf")?.strip_suffix('»')?;
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{mac_png, pick};

    #[test]
    fn a_known_image_type_is_picked_in_order() {
        assert_eq!(
            pick("text/plain\nimage/jpeg\nimage/png\n"),
            Some(("image/png", "png"))
        );
        assert_eq!(pick("image/webp"), Some(("image/webp", "webp")));
        assert_eq!(pick("text/plain\nUTF8_STRING\nTARGETS"), None, "只有字");
    }

    #[test]
    fn the_mac_clipboard_hex_turns_into_bytes() {
        assert_eq!(
            mac_png("«data PNGf89504E47»\n"),
            Some(vec![0x89, 0x50, 0x4e, 0x47])
        );
        assert_eq!(mac_png("你好"), None, "剪贴板里是字");
        assert_eq!(mac_png("«data PNGf8950Z»"), None, "不是十六进制");
    }
}
