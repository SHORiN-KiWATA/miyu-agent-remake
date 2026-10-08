//! `miyu-embed --manifest <清单> --dir <模型文件的目录>`（施工 R-5 上，`docs/blueprint/recall.md` 第四条第 4 款）。
//!
//! 载入成了，标准输出先印一行 `{"ready":{"model":"local:<id>","dims":<维数>}}`，然后照 [`miyu_embed::serve`] 一行一句地答，
//! 标准输入关了退出码 0。清单读不懂、文件没有、模型载入不了：标准输出印一行 `{"error":"…"}`，退出码 1。参数写错：标准错误
//! 印用法，退出码 2。

use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use serde_json::json;

use miyu_embed::manifest::Manifest;
use miyu_embed::model::Embedder;
use miyu_embed::serve::serve;

/// 参数写错时印的用法。
const USAGE: &str = "usage: miyu-embed --manifest <file> --dir <directory>";

fn main() -> ExitCode {
    let Some((manifest, dir)) = arguments(std::env::args_os().skip(1).collect()) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let loaded = Manifest::read(&manifest)
        .map_err(|error| error.to_string())
        .and_then(|manifest| Embedder::load(manifest, &dir).map_err(|error| error.to_string()));
    let mut out = std::io::stdout().lock();
    let mut embedder = match loaded {
        Ok(embedder) => embedder,
        Err(error) => {
            // 核心照这一行看为什么起不来；写不出去的，原因写到标准错误。
            if let Err(lost) = writeln!(out, "{}", json!({ "error": error })) {
                eprintln!("{error} ({lost})");
            }
            return ExitCode::from(1);
        }
    };
    let ready = json!({"ready": {"model": embedder.model(), "dims": embedder.dims()}});
    let served = writeln!(out, "{ready}")
        .and_then(|()| out.flush())
        .and_then(|()| serve(&mut embedder, std::io::stdin().lock(), &mut out));
    match served {
        Ok(()) => ExitCode::SUCCESS,
        // 核心那一头关了、读不了标准输入：原因写到标准错误，退出码说明不是正常读完。
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

/// 读 `--manifest <清单> --dir <目录>`，先后不论，各正好一次，别的都不认。
fn arguments(args: Vec<OsString>) -> Option<(PathBuf, PathBuf)> {
    let mut manifest = None;
    let mut dir = None;
    let mut args = args.into_iter();
    while let Some(flag) = args.next() {
        let slot = match flag.to_str() {
            Some("--manifest") => &mut manifest,
            Some("--dir") => &mut dir,
            _ => return None,
        };
        if slot.replace(PathBuf::from(args.next()?)).is_some() {
            return None;
        }
    }
    Some((manifest?, dir?))
}
