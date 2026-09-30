//! `miyu config edit`（`config.md` 第十条第 6 条，施工 8-3）：像 visudo，用编辑器打开，存盘时先检查。
//!
//! 1. 标准输入、标准错误不是终端：说一句，退出码 2（连核心以前就拦下了，`config.rs` 的 `early`）。
//! 2. `config.get` 拿到这一层的文件在哪，读它现在的字和版本（磁盘上的）。没有文件的是空的；`--project` 还没有的照仓库的根。
//! 3. 在原文件旁边写一份副本 `.<文件名>.edit-<随机>.toml`（Schema 的相对路径照样对），Unix 上 0600。
//! 4. 开编辑器。退出码不是 0：说一句，删掉副本，退出码 1。字没变：说「没改」，删掉副本，退出码 0。
//! 5. 变了：`config.check`。有错误：印出每一条，问「回车接着改，输入 q 放弃」；回车再开编辑器，改的还在；q（读到头也算）删掉
//!    副本、说放弃了，退出码 1。只有警告的：印出来，照样存。
//! 6. 存：系统配置、个人设置经 `config.set`，带 `text` 和第 2 步的版本；项目配置核心不写，命令行自己照改配置的规矩写回
//!    （顺着链接、临时文件、替换前再读）。别处改过了：说一句，副本留着、说它在哪，退出码 1。成了：删掉副本，印一行灰字。
//!
//! 为了副本新建的目录（还没有的 `home/<账号>/`、仓库里的 `.miyu/`），没存的时候删掉，空的才删。

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use miyu_store::config_file::{self, WriteError};

use super::check::Only;
use super::console::Console;
use super::paths::{self, Places};
use super::render;
use super::{Reply, Talk};
use crate::exit;
use crate::shown::{Line, say, tilde, write};

/// 打开来改的一份：在哪、给人看的写法、原来的字和版本、BOM，副本在哪、为它新建的目录。
struct Editing {
    path: PathBuf,
    shown: String,
    original: String,
    version: Option<String>,
    bom: bool,
    copy: PathBuf,
    made: Option<PathBuf>,
}

impl Editing {
    /// 不存了：删掉副本，为它新建的目录空了也删。
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留着：副本点开头，目录空着不碍事"
    )]
    fn discard(&self) {
        let _ = fs::remove_file(&self.copy);
        if let Some(dir) = &self.made {
            let _ = fs::remove_dir(dir);
        }
    }
}

/// 用编辑器改一层，交回退出码。
pub(super) async fn edit(talk: &mut Talk<'_>, only: Only, console: &mut dyn Console) -> u8 {
    let language = talk.plan.language;
    let editing = match open(talk, only).await {
        Ok(editing) => editing,
        Err(code) => return code,
    };
    let Some(text) = edited(talk, only, console, &editing).await else {
        return exit::ERROR;
    };
    let text = match text {
        Some(text) => text,
        None => {
            editing.discard();
            say(talk.err, language.nothing_changed());
            return exit::OK;
        }
    };
    save(talk, only, &editing, &text).await
}

/// 第 2、3 步：找到这一层的文件、读它现在的字，在旁边写好副本。
async fn open(talk: &mut Talk<'_>, only: Only) -> Result<Editing, u8> {
    let params = match only {
        Only::Project => json!({"cwd": talk.cwd()}),
        _ => json!({}),
    };
    let files = talk.ask("config.get", params).await?["files"].clone();
    let places = Places::of(talk.plan);
    let path = match files[layer(only)]["file"].as_str() {
        Some(file) => places.absolute(file),
        None => paths::planned(&talk.plan.cwd),
    };
    let shown = tilde(&path.to_string_lossy(), talk.plan.home.as_deref());
    let read = match config_file::read(&path) {
        Ok(read) => read,
        Err(error) => {
            say(talk.err, &format!("{shown}: {error}"));
            return Err(exit::ERROR);
        }
    };
    let (original, version, bom) = match read {
        Some(read) => (read.text, Some(read.version), read.bom),
        None => (String::new(), None, false),
    };
    let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
    let made = (!dir.exists()).then(|| dir.clone());
    let copy = fs::create_dir_all(&dir).and_then(|()| copy(&path, &original));
    match copy {
        Ok(copy) => Ok(Editing {
            path,
            shown,
            original,
            version,
            bom,
            copy,
            made,
        }),
        Err(error) => {
            say(talk.err, &format!("{shown}: {error}"));
            Err(exit::ERROR)
        }
    }
}

/// 第 4、5 步：开编辑器、查，直到没有错误或者人放弃。交回改好的字；没改的是 `Some(None)`；编辑器出错、放弃了是空的（话
/// 已经说了、副本删了）。
async fn edited(
    talk: &mut Talk<'_>,
    only: Only,
    console: &mut dyn Console,
    editing: &Editing,
) -> Option<Option<String>> {
    let language = talk.plan.language;
    loop {
        match console.edit(&editing.copy) {
            Ok(Some(0)) => {}
            Ok(code) => {
                editing.discard();
                say(talk.err, &language.editor_failed(code));
                return None;
            }
            Err(error) => {
                editing.discard();
                say(talk.err, &error.to_string());
                return None;
            }
        }
        let text = match config_file::read(&editing.copy) {
            Ok(Some(read)) => read.text,
            Ok(None) => String::new(),
            Err(error) => {
                say(talk.err, &error.to_string());
                String::new()
            }
        };
        if text == editing.original {
            return Some(None);
        }
        let params = json!({"layer": layer(only), "text": text});
        let problems = match talk.ask("config.check", params).await {
            Ok(result) => result["problems"].as_array().cloned().unwrap_or_default(),
            Err(_) => {
                editing.discard();
                return None;
            }
        };
        for problem in &problems {
            let line = render::problem(&editing.shown, problem, language);
            write(talk.err, &line.paint(talk.plan.gray));
        }
        let errors = problems
            .iter()
            .filter(|problem| problem["level"] != "warning")
            .count();
        if errors == 0 {
            return Some(Some(text));
        }
        write(talk.err, &language.edit_errors(errors));
        match console.line() {
            Ok(Some(answer)) if answer.trim() != "q" => {}
            _ => {
                editing.discard();
                say(talk.err, language.gave_up());
                return None;
            }
        }
    }
}

/// 第 6 步：存。
async fn save(talk: &mut Talk<'_>, only: Only, editing: &Editing, text: &str) -> u8 {
    let language = talk.plan.language;
    let copy = tilde(&editing.copy.to_string_lossy(), talk.plan.home.as_deref());
    let applies = match only {
        Only::Project => {
            let bytes = config_file::bytes(text, editing.bom);
            match config_file::write(&editing.path, &bytes, editing.version.as_deref()) {
                Ok(()) => Vec::new(),
                Err(WriteError::Changed) => {
                    say(talk.err, &language.edit_conflict(&copy));
                    return exit::ERROR;
                }
                Err(WriteError::Io(error)) => {
                    say(talk.err, &format!("{}: {error}", editing.shown));
                    return exit::ERROR;
                }
            }
        }
        _ => {
            let params = json!({"layer": layer(only), "text": text, "version": editing.version});
            match talk.request("config.set", params).await {
                Ok(Reply::Done(result)) => applies(&result["keys"]),
                Ok(Reply::Refused(error)) if error["data"]["reason"] == "config_conflict" => {
                    say(talk.err, &language.edit_conflict(&copy));
                    return exit::ERROR;
                }
                Ok(Reply::Refused(error)) => {
                    editing.discard();
                    return talk.refused(&error);
                }
                Err(code) => return code,
            }
        }
    };
    editing.discard();
    let applies: Vec<&str> = applies.iter().map(String::as_str).collect();
    write(
        talk.err,
        &Line::gray(language.edit_saved(&applies)).paint(talk.plan.gray),
    );
    exit::OK
}

/// 改了的几项什么时候生效，照键名的先后，一样的只算一次。
fn applies(keys: &Value) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for entry in keys.as_object().into_iter().flat_map(|keys| keys.values()) {
        if let Some(applies) = entry["applies"].as_str()
            && !found.iter().any(|seen| seen == applies)
        {
            found.push(applies.to_string());
        }
    }
    found
}

/// 改哪一层：`--system` 系统配置，`--project` 项目配置，都不写的个人设置。
fn layer(only: Only) -> &'static str {
    match only {
        Only::System => "system",
        Only::Project => "project",
        Only::All => "personal",
    }
}

/// 在 `path` 旁边写一份副本，名字 `.<文件名>.edit-<随机>.toml`，只许新建，Unix 上 0600。
fn copy(path: &Path, text: &str) -> std::io::Result<PathBuf> {
    let name = path
        .file_name()
        .map_or_else(|| "config.toml".into(), |name| name.to_string_lossy());
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut tries = 0;
    loop {
        let copy = dir.join(format!(".{name}.edit-{}.toml", random()));
        match options.open(&copy) {
            Ok(mut file) => {
                file.write_all(text.as_bytes())?;
                return Ok(copy);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists && tries < 8 => {
                tries += 1;
            }
            Err(error) => return Err(error),
        }
    }
}

/// 副本名字里的随机一段：4 个随机字节写成 8 位十六进制；取不到随机数的，用进程号和此刻的纳秒。
fn random() -> String {
    let mut bytes = [0u8; 4];
    match getrandom::fill(&mut bytes) {
        Ok(()) => bytes.iter().map(|byte| format!("{byte:02x}")).collect(),
        Err(_) => {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| since.subsec_nanos());
            format!("{:x}{nanos:x}", std::process::id())
        }
    }
}
