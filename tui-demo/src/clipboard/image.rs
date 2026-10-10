//! `Ctrl+V` 读系统剪贴板里的图、复制的文件（蓝图 `tui.md`「输入框」第 12 条，2026-10-10 项目主人：「ctrl+shift+V 粘贴文字，
//! ctrl+V 粘贴占位符」）：先看复制的是不是文件（`text/uri-list`，文件管理器里复制的），是的交回那几行，由输入框收成块；
//! 再看有没有图（附了字也照图），有的存一份到给的目录（照内容起名，同一张不存两份），收成附件；都没有的照旧读字。
//!
//! Linux 有 Wayland、装了 `wl-paste` 的只问它（`--list-types`），没有 Wayland、没装 `wl-paste` 才问 `xclip` 的 `TARGETS`；
//! macOS 用 `osascript`：复制的是文件（`«class furl»`）的不当图，取 `«class PNGf»`。每个命令最多等一会儿，卡住的放弃
//! （`read.rs`）。

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

/// 复制的文件那一种。
const FILES: &str = "text/uri-list";

/// 剪贴板里有图（不是复制的文件）的，存进 `dir`，交回文件；没有图、读不到、存不下的交回 `None`。
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

/// 剪贴板里是复制的文件：交回 `text/uri-list` 那几行（`file://` 开头，一行一个）；不是的、读不到的交回 `None`。
/// macOS 不读（Finder 复制的文件另有一套，先不做）。
pub fn read_files() -> Option<String> {
    if cfg!(target_os = "macos") || cfg!(windows) {
        return None;
    }
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    linux_files(wayland, &System)
}

/// 照这台机器的剪贴板命令取图：字节和扩展名。
fn grab() -> Option<(Vec<u8>, &'static str)> {
    if cfg!(target_os = "macos") {
        let info = run_bytes("osascript", &["-e", "clipboard info"], WAIT)?;
        if mac_has_files(&String::from_utf8_lossy(&info)) {
            return None;
        }
        let out = run_bytes("osascript", &["-e", "the clipboard as «class PNGf»"], WAIT)?;
        return mac_png(&String::from_utf8_lossy(&out)).map(|png| (png, "png"));
    }
    if cfg!(windows) {
        return None;
    }
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    linux_image(wayland, &System)
}

/// 跑剪贴板命令（测试里换成假的）。
trait Runner {
    /// 这个命令装了。
    fn has(&self, cmd: &str) -> bool;
    /// 跑一下读输出；失败、卡住的是 `None`。
    fn run(&self, cmd: &str, args: &[&str]) -> Option<Vec<u8>>;
}

/// 真的去跑。
struct System;

impl Runner for System {
    fn has(&self, cmd: &str) -> bool {
        installed(cmd)
    }

    fn run(&self, cmd: &str, args: &[&str]) -> Option<Vec<u8>> {
        run_bytes(cmd, args, WAIT)
    }
}

/// 问哪个命令、怎么读一种：有 Wayland、装了 `wl-paste` 的只问它（是字、是空的都不再去问 X11）；没有 Wayland、没装
/// `wl-paste` 才问 X11 的 `xclip`（2026-10-10 项目主人报：复制了磁力链接，`Ctrl+V` 还是粘出一张图——X11 那一侧卡着
/// 旧图）。交回命令、剪贴板说它有的几种（一行一种）、读一种的参数。
type Reader = fn(&str) -> Vec<&str>;

fn offered(wayland: bool, run: &dyn Runner) -> Option<(&'static str, String, Reader)> {
    let (cmd, read, list): (&str, Reader, Vec<&str>) = if wayland && run.has("wl-paste") {
        ("wl-paste", wl_paste, vec!["--list-types"])
    } else {
        ("xclip", xclip, xclip("TARGETS"))
    };
    let types = run.run(cmd, &list)?;
    Some((cmd, String::from_utf8_lossy(&types).into_owned(), read))
}

/// Linux 取图：复制的是文件的不算（交给 [`linux_files`]），附了字也照图。
fn linux_image(wayland: bool, run: &dyn Runner) -> Option<(Vec<u8>, &'static str)> {
    let (cmd, types, read) = offered(wayland, run)?;
    let (mime, ext) = pick(&types)?;
    let bytes = run.run(cmd, &read(mime))?;
    (!bytes.is_empty()).then_some((bytes, ext))
}

/// Linux 取复制的文件：`text/uri-list` 那几行。
fn linux_files(wayland: bool, run: &dyn Runner) -> Option<String> {
    let (cmd, types, read) = offered(wayland, run)?;
    if !types.lines().any(|t| t.trim() == FILES) {
        return None;
    }
    let bytes = run.run(cmd, &read(FILES))?;
    let text = String::from_utf8_lossy(&bytes).trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// `wl-paste` 读 `mime` 那一种的参数。
fn wl_paste(mime: &str) -> Vec<&str> {
    vec!["--type", mime]
}

/// `xclip` 读剪贴板里 `mime` 那一种（`TARGETS` 是有哪几种）的参数。
fn xclip(mime: &str) -> Vec<&str> {
    vec!["-selection", "clipboard", "-t", mime, "-o"]
}

/// `cmd` 在 `PATH` 里找得到。
fn installed(cmd: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(cmd).is_file()))
}

/// 剪贴板说它有的几种（一行一种）里挑一种认得的图：附了字也挑（`Ctrl+V` 贴占位符，字用 `Ctrl+Shift+V`）；复制的是文件
/// 的不挑（文件照文件收）。
fn pick(types: &str) -> Option<(&'static str, &'static str)> {
    let have: Vec<&str> = types.lines().map(str::trim).collect();
    if have.contains(&FILES) {
        return None;
    }
    KINDS.into_iter().find(|(mime, _)| have.contains(mime))
}

/// macOS 的 `clipboard info`（`«class furl», 60, «class icns», 3000…`）里有复制的文件：Finder 复制文件时另附一张图标，
/// 不当图收。
fn mac_has_files(info: &str) -> bool {
    info.split(',').map(str::trim).any(|t| t == "«class furl»")
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
    use super::{Runner, linux_files, linux_image, mac_has_files, mac_png, pick};

    /// 假的剪贴板：Wayland 那一侧是字，X11 那一侧卡着一张旧图。
    struct Stale;

    impl Runner for Stale {
        fn has(&self, cmd: &str) -> bool {
            matches!(cmd, "wl-paste" | "xclip")
        }

        fn run(&self, cmd: &str, args: &[&str]) -> Option<Vec<u8>> {
            let out: &[u8] = match (cmd, args.last().copied()) {
                ("wl-paste", Some("--list-types")) => b"text/plain;charset=utf-8\ntext/plain\n",
                ("xclip", Some("-o")) if args.contains(&"TARGETS") => b"TARGETS\nimage/png\n",
                ("xclip", Some("-o")) => b"\x89PNG old",
                _ => return None,
            };
            Some(out.to_vec())
        }
    }

    #[test]
    fn when_wayland_says_text_the_stale_x11_image_is_not_asked() {
        // 2026-10-10 项目主人报：复制了磁力链接，Ctrl+V 还是粘出一张图（X11 那一侧卡着旧图）。
        assert_eq!(linux_image(true, &Stale), None, "Wayland 答了是字，就照字");
        assert!(
            linux_image(false, &Stale).is_some(),
            "没有 Wayland 的照 X11"
        );
    }

    #[test]
    fn a_known_image_type_is_picked_in_order() {
        assert_eq!(pick("image/jpeg\nimage/png\n"), Some(("image/png", "png")));
        assert_eq!(pick("image/webp"), Some(("image/webp", "webp")));
        assert_eq!(pick("text/plain\nUTF8_STRING\nTARGETS"), None, "只有字");
        assert_eq!(
            pick("image/png\ntext/html"),
            Some(("image/png", "png")),
            "浏览器复制图片：带一份 HTML，不算字"
        );
    }

    /// 假的剪贴板：文件管理器里复制的一张图（`text/uri-list`，另附一份图）。
    struct Copied;

    impl Runner for Copied {
        fn has(&self, cmd: &str) -> bool {
            cmd == "wl-paste"
        }

        fn run(&self, cmd: &str, args: &[&str]) -> Option<Vec<u8>> {
            let out: &[u8] = match (cmd, args.last().copied()) {
                ("wl-paste", Some("--list-types")) => b"text/uri-list\ntext/plain\nimage/png\n",
                ("wl-paste", Some("text/uri-list")) => b"file:///tmp/a.png\r\n",
                ("wl-paste", Some("image/png")) => b"\x89PNG",
                _ => return None,
            };
            Some(out.to_vec())
        }
    }

    #[test]
    fn ctrl_v_takes_copied_files_then_pictures_then_text() {
        // 2026-10-10 项目主人：「ctrl+shift+V 粘贴文字，ctrl+V 粘贴占位符」。附了字的图照图（原来 10-09 定的是有字照字，
        // 那一回粘出图的根子是 X11 那一侧卡着旧图，上一个测试守着）。
        assert_eq!(
            pick("text/plain;charset=utf-8\ntext/plain\nimage/png"),
            Some(("image/png", "png")),
            "附了字也照图"
        );
        assert_eq!(
            pick("text/uri-list\nimage/png"),
            None,
            "复制的是文件：照文件收"
        );
        assert_eq!(
            linux_files(true, &Copied).as_deref(),
            Some("file:///tmp/a.png")
        );
        assert_eq!(linux_image(true, &Copied), None);
        assert_eq!(
            linux_files(true, &super::tests::Stale),
            None,
            "只有字的不是文件"
        );
        assert!(mac_has_files("«class furl», 60, «class icns», 3000"));
        assert!(!mac_has_files("«class PNGf», 120, «class utf8», 20"));
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
