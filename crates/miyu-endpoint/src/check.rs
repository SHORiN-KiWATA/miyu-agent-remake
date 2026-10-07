//! `check`：一次查完人手写的、Miyu 读的文件（施工 8-30，`docs/blueprint/protocol.md`「`check`」，`cli/check.md`）。
//!
//! 照磁盘上现在的字查，不是核心手里的那一份：人刚改完、核心还没重读的也查得到。不写文件的查全部：系统配置、管理员的
//! 个人设置、`cwd` 的项目配置（没写 `cwd` 的不查）、密钥文件（照核心手里的问题：它的字是密钥，不另读）、三层里每个
//! 人格的两份字（每一层各查各的）。写了文件的照它在哪认是哪一种，只查那一份；认不出的 `unknown_file`。
//!
//! 一处一格：`kind`（`config`、`secrets`、`persona`）、`file`（照 `config.get` 的写法：数据根里的写成相对数据根的，
//! 项目配置写成 `~/…`，出厂的人格写真的路径）、`code`、`level`、`message`（照这个连接的语言），有行列的带上。

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::{Layer, Words};
use miyu_store::config_file::{self, ReadError};
use miyu_store::human::Human;
use miyu_store::personas::{Checked, Issue};

use crate::Core;
use crate::config::methods::{check_text, said_at, words};
use crate::hello::Peer;
use crate::personas::personas;
use crate::refusal::Refusal;

/// `check` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CheckParams {
    /// 头现在的工作目录：照它找项目配置；写了相对的 `file` 照它接。
    #[serde(default)]
    cwd: Option<String>,
    /// 只查这一份。
    #[serde(default)]
    file: Option<String>,
}

/// 一份配置文件：哪一层、真的路径、给人看的写法。
struct ConfigFile {
    layer: Layer,
    path: PathBuf,
    shown: String,
}

/// `check`：交回 `{"problems": [...]}`。
pub(crate) async fn check(core: &Core, peer: Peer, params: CheckParams) -> Result<Value, Refusal> {
    let words = words(core, peer.language)?;
    let home = core.home.clone();
    let cwd = params
        .cwd
        .as_deref()
        .map(|cwd| expand(cwd, home.as_deref(), None));
    let (configs, secrets_path, secrets) = config_files(core, &words, cwd.as_deref())?;
    let mut problems = Vec::new();
    match params.file {
        None => {
            for file in &configs {
                problems.extend(check_config(core, &words, file)?);
            }
            problems.extend(secrets);
            let personas = personas(core);
            let checked = tokio::task::spawn_blocking(move || personas.check())
                .await
                .map_err(|_| Refusal::INTERNAL)?;
            problems.extend(checked.iter().map(|one| persona_problem(core, &words, one)));
        }
        Some(file) => {
            let path = real(&expand(&file, home.as_deref(), cwd.as_deref()));
            if let Some(file) = configs.iter().find(|config| real(&config.path) == path) {
                problems.extend(check_config(core, &words, file)?);
            } else if let Some(file) = project_at(&path, home.as_deref()) {
                problems.extend(check_config(core, &words, &file)?);
            } else if real(&secrets_path) == path {
                problems.extend(secrets);
            } else {
                let personas = personas(core);
                let wanted = path.clone();
                let checked = tokio::task::spawn_blocking(move || personas.check_file(&wanted))
                    .await
                    .map_err(|_| Refusal::INTERNAL)?;
                let Some(checked) = checked else {
                    return Err(Refusal::UNKNOWN_FILE);
                };
                problems.extend(checked.iter().map(|one| persona_problem(core, &words, one)));
            }
        }
    }
    Ok(json!({ "problems": problems }))
}

/// 几份配置文件，和密钥文件的位置、它的问题（照核心手里的那一份）。
fn config_files(
    core: &Core,
    words: &Human,
    cwd: Option<&Path>,
) -> Result<(Vec<ConfigFile>, PathBuf, Vec<Value>), Refusal> {
    let config = core.config();
    let mut files: Vec<ConfigFile> = [&config.system, &config.personal]
        .into_iter()
        .map(|file| ConfigFile {
            layer: file.layer,
            path: file.path.clone(),
            shown: file.shown.clone(),
        })
        .collect();
    let project = cwd.and_then(|cwd| config.project(&cwd.to_string_lossy()));
    if let Some(project) = &project {
        files.push(ConfigFile {
            layer: Layer::Project,
            path: project.file.path.clone(),
            shown: project.file.shown.clone(),
        });
    }
    let layers = config.layers(project.as_ref());
    let secrets = &config.secrets;
    let place = Some((secrets.shown.as_str(), secrets.last_good));
    let mut problems = Vec::new();
    for problem in secrets.problems() {
        let mut said = said_at(&config, &layers, problem, place, words)?;
        said["kind"] = json!("secrets");
        problems.push(said);
    }
    Ok((files, secrets.path.clone(), problems))
}

/// 查一份配置文件磁盘上现在的字；还没有的不报，读不了的报一条。
fn check_config(core: &Core, words: &Human, file: &ConfigFile) -> Result<Vec<Value>, Refusal> {
    let text = match config_file::read(&file.path) {
        Ok(Some(text)) => text.text,
        Ok(None) => return Ok(Vec::new()),
        Err(error) => return Ok(vec![unreadable(words, &file.shown, "config", &error)]),
    };
    let mut problems = check_text(core, words, file.layer, &text)?;
    for problem in &mut problems {
        problem["file"] = json!(file.shown);
        problem["kind"] = json!("config");
    }
    Ok(problems)
}

/// 写了文件、它是某个目录下的 `.miyu/config.toml`：当项目配置查（不在 `cwd` 下面也行）。
fn project_at(path: &Path, home: Option<&Path>) -> Option<ConfigFile> {
    let name = path.file_name()?.to_str()?;
    let parent = path.parent()?;
    (name == "config.toml" && parent.file_name()?.to_str()? == ".miyu").then(|| ConfigFile {
        layer: Layer::Project,
        path: path.to_path_buf(),
        shown: tilde(path, home),
    })
}

/// 命令行读不了一份文件的那一条：照核心的说法。
fn unreadable(words: &Human, shown: &str, kind: &str, error: &ReadError) -> Value {
    let (code, message) = match error {
        ReadError::Unreadable(why) => (
            "unreadable",
            Words::sentence(words, "config/unreadable", &[("why", &why.to_string())]),
        ),
        ReadError::TooBig => ("too_big", Words::sentence(words, "config/too-big", &[])),
        ReadError::NotUtf8 => ("not_utf8", Words::sentence(words, "config/not-utf8", &[])),
    };
    json!({"kind": kind, "file": shown, "code": code, "level": "error", "message": message.unwrap_or_default()})
}

/// 人格的一处。
fn persona_problem(core: &Core, words: &Human, checked: &Checked) -> Value {
    let shown = match checked.path.strip_prefix(core.root.path()) {
        Ok(rest) => rest
            .iter()
            .map(|part| part.to_string_lossy())
            .collect::<Vec<_>>()
            .join("/"),
        Err(_) => checked.path.to_string_lossy().into_owned(),
    };
    match &checked.issue {
        Issue::Wrong(problem) => {
            let key = format!("persona-problems/{}", problem.code.as_str());
            let message = Words::sentence(words, &key, &[("detail", &problem.detail)]);
            let mut said = json!({
                "kind": "persona",
                "file": shown,
                "code": problem.code.as_str(),
                "level": "error",
                "message": message.unwrap_or_else(|| problem.message.clone()),
            });
            if let Some(line) = problem.line {
                said["line"] = json!(line);
            }
            said
        }
        Issue::Unreadable(error) => {
            let why = ReadError::Unreadable(std::io::Error::new(error.kind(), error.to_string()));
            unreadable(words, &shown, "persona", &why)
        }
    }
}

/// `~`、`~/` 开头的照家目录接，相对的照 `cwd` 接，绝对的照原样。
fn expand(text: &str, home: Option<&Path>, cwd: Option<&Path>) -> PathBuf {
    let rest = match text {
        "~" => Some(""),
        _ => text.strip_prefix("~/"),
    };
    if let (Some(rest), Some(home)) = (rest, home) {
        return home.join(rest);
    }
    let path = Path::new(text);
    match (path.is_absolute(), cwd) {
        (false, Some(cwd)) => cwd.join(path),
        _ => path.to_path_buf(),
    }
}

/// 真的位置（[`miyu_fs::resolve`]：还没有的照最近一层在的上级换，再接上后面几段；macOS 的 `/var` 是链接，Windows 换出来带
/// `\\?\`，只换一边就对不上）。换不成的照原样。
fn real(path: &Path) -> PathBuf {
    miyu_fs::resolve(Path::new("/"), None, &path.to_string_lossy())
        .unwrap_or_else(|_| path.to_path_buf())
}

/// 家目录下的写成 `~/…`。
fn tilde(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) => format!("~/{}", rest.to_string_lossy()),
        None => path.to_string_lossy().into_owned(),
    }
}
