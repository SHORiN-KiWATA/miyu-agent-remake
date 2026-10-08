//! 选目录（`/pick-dir`）：在桥这台机器上开系统的选目录对话框，交回选的绝对路径（蓝图 `web.md`「人格、预设、工作区」第 4 条；
//! 2026-10-08 项目主人：选择文件夹应该打开目录选择器）。浏览器给不了本机路径；桥只听 127.0.0.1，和页面在同一台机器上，由它开。
//!
//! - Linux：依次试 `zenity`（GTK）、`kdialog`（KDE），都没有的交「开不了」，页面退回自己画的文件夹浏览器；
//! - macOS：`osascript` 的 `choose folder`；
//! - Windows：PowerShell 的 `FolderBrowserDialog`。
//!
//! 标题、从哪个目录开始经环境变量交给对话框程序，不拼进命令行（不用管引号、转义）。点了取消的交「取消了」。

use std::io::ErrorKind;
use std::process::Stdio;

use tokio::process::Command;

/// 对话框的结果。
pub enum Picked {
    /// 选了这个目录（绝对路径）。
    Path(String),
    /// 点了取消。
    Cancelled,
    /// 这台机器上开不了（没有对话框程序）。
    Unavailable,
}

impl Picked {
    /// 回给页面的 JSON：`{"path"}`、`{"cancelled": true}`、`{"unavailable": true}`。
    pub fn json(&self) -> String {
        match self {
            Picked::Path(p) => serde_json::json!({ "path": p }).to_string(),
            Picked::Cancelled => r#"{"cancelled":true}"#.to_owned(),
            Picked::Unavailable => r#"{"unavailable":true}"#.to_owned(),
        }
    }
}

/// 一种对话框程序：程序名、参数（`$MIYU_PICK_TITLE`、`$MIYU_PICK_START` 由程序自己读）。
struct Tool {
    program: &'static str,
    args: Vec<String>,
}

/// 这个系统上依次试哪几种。
fn tools(start: Option<&str>) -> Vec<Tool> {
    if cfg!(target_os = "macos") {
        let script = if start.is_some() {
            "POSIX path of (choose folder with prompt (system attribute \"MIYU_PICK_TITLE\") default location (POSIX file (system attribute \"MIYU_PICK_START\")))"
        } else {
            "POSIX path of (choose folder with prompt (system attribute \"MIYU_PICK_TITLE\"))"
        };
        return vec![Tool { program: "osascript", args: vec!["-e".into(), script.into()] }];
    }
    if cfg!(windows) {
        let script = "[Console]::OutputEncoding = [Text.Encoding]::UTF8; Add-Type -AssemblyName System.Windows.Forms; \
            $d = New-Object System.Windows.Forms.FolderBrowserDialog; $d.Description = $env:MIYU_PICK_TITLE; \
            if ($env:MIYU_PICK_START) { $d.SelectedPath = $env:MIYU_PICK_START }; \
            if ($d.ShowDialog() -eq 'OK') { [Console]::Out.Write($d.SelectedPath) } else { exit 1 }";
        return vec![Tool { program: "powershell", args: vec!["-NoProfile".into(), "-STA".into(), "-Command".into(), script.into()] }];
    }
    // zenity 从哪开始照 `--filename`，目录要带结尾的 `/`；kdialog 照第一个参数
    let mut zenity = vec!["--file-selection".into(), "--directory".into(), "--title".into(), "$MIYU_PICK_TITLE".into()];
    let mut kdialog = vec!["--getexistingdirectory".into()];
    if let Some(dir) = start {
        zenity.extend(["--filename".into(), format!("{}/", dir.trim_end_matches('/'))]);
        kdialog.push(dir.into());
    }
    kdialog.extend(["--title".into(), "$MIYU_PICK_TITLE".into()]);
    vec![Tool { program: "zenity", args: zenity }, Tool { program: "kdialog", args: kdialog }]
}

/// 开对话框，等人选完。
pub async fn pick_dir(title: &str, start: Option<&str>) -> Picked {
    for tool in tools(start) {
        // 标题经环境变量交：参数里写的 `$MIYU_PICK_TITLE` 在这里换成真的字（不经 shell）
        let args: Vec<String> = tool.args.iter().map(|a| if a == "$MIYU_PICK_TITLE" { title.to_owned() } else { a.clone() }).collect();
        let mut cmd = Command::new(tool.program);
        cmd.args(&args).env("MIYU_PICK_TITLE", title).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).kill_on_drop(true);
        if let Some(dir) = start {
            cmd.env("MIYU_PICK_START", dir);
        }
        let out = match cmd.output().await {
            Ok(out) => out,
            Err(e) if e.kind() == ErrorKind::NotFound => continue,
            Err(e) => {
                eprintln!("开不了选目录的对话框（{}）：{e}", tool.program);
                continue;
            }
        };
        if !out.status.success() {
            return Picked::Cancelled;
        }
        let path = String::from_utf8_lossy(&out.stdout).trim_end_matches(['\r', '\n']).to_owned();
        // osascript 交的目录带结尾的 `/`；根目录留着
        let path = if path.len() > 1 { path.trim_end_matches('/').to_owned() } else { path };
        return if path.is_empty() { Picked::Cancelled } else { Picked::Path(path) };
    }
    Picked::Unavailable
}
