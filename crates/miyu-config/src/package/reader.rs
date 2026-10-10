//! 一张表一张表地读（施工 9-1 上，从 `package.rs` 挪出来：那边放不下了）：每一格的写法、报在哪一行。

use toml_edit::{Item, TableLike};

use super::{Capability, Code, Command, PackageKind, Pages, Problem, Process, Start};
use crate::phrases::{self, PhraseError, Phrases};

/// `[package]` 读出来的几格。
pub(super) struct Head {
    pub(super) kind: PackageKind,
    pub(super) version: Option<String>,
    pub(super) protocol: [u32; 2],
    pub(super) name: Phrases,
    pub(super) summary: Phrases,
    pub(super) required: bool,
    pub(super) icon: Option<String>,
}

/// 照原文算行号。
pub(super) struct Reader<'a> {
    pub(super) text: &'a str,
}

impl Reader<'_> {
    /// 一处问题：`item` 在第几行（没有的整份）。
    pub(super) fn problem(
        &self,
        item: Option<&Item>,
        code: Code,
        detail: &str,
        message: String,
    ) -> Problem {
        Problem {
            line: item
                .and_then(Item::span)
                .map(|span| line_of(self.text, span.start)),
            code,
            detail: detail.to_string(),
            message,
        }
    }

    /// 表 `table`（叫 `name`）里只能有 `keys`。
    pub(super) fn only(
        &self,
        table: &dyn TableLike,
        name: &str,
        keys: &[&str],
    ) -> Result<(), Problem> {
        for (key, item) in table.iter() {
            if !keys.contains(&key) {
                return Err(self.problem(
                    Some(item),
                    Code::UnknownKey,
                    &format!("{name}.{key}"),
                    format!("{name}.{key} is not a known key"),
                ));
            }
        }
        Ok(())
    }

    /// 必写的一格：没有的报在表头那一行。
    pub(super) fn required<'t>(
        &self,
        table: &'t dyn TableLike,
        at: &Item,
        name: &str,
        key: &str,
    ) -> Result<&'t Item, Problem> {
        table.get(key).ok_or_else(|| {
            self.problem(
                Some(at),
                Code::MissingKey,
                &format!("{name}.{key}"),
                format!("{name}.{key} is missing"),
            )
        })
    }

    /// `[package]`。
    pub(super) fn package(&self, table: &dyn TableLike, at: &Item) -> Result<Head, Problem> {
        self.only(
            table,
            "package",
            &[
                "kind", "version", "protocol", "name", "summary", "required", "icon",
            ],
        )?;
        let item = self.required(table, at, "package", "kind")?;
        let kind = match item.as_str() {
            Some("ui") => PackageKind::Ui,
            Some("process") => PackageKind::Process,
            Some("builtin") => PackageKind::Builtin,
            Some("worker") => PackageKind::Worker,
            other => {
                let shown = other.map_or_else(
                    || {
                        item.span()
                            .and_then(|span| self.text.get(span))
                            .unwrap_or_default()
                            .trim()
                            .to_string()
                    },
                    str::to_string,
                );
                return Err(self.problem(
                    Some(item),
                    Code::BadKind,
                    &shown,
                    format!("package.kind must be ui, process, builtin or worker, not \"{shown}\""),
                ));
            }
        };
        let version = match table.get("version") {
            Some(item) => Some(self.text(item, "package.version")?),
            None => None,
        };
        let item = self.required(table, at, "package", "protocol")?;
        let protocol = protocol(item).ok_or_else(|| {
            self.problem(
                Some(item),
                Code::BadProtocol,
                "package.protocol",
                "package.protocol must be two non-negative integers [lowest, highest]".to_string(),
            )
        })?;
        let name = self.phrases(self.required(table, at, "package", "name")?, "package.name")?;
        let summary = match table.get("summary") {
            Some(item) => self.phrases(item, "package.summary")?,
            None => Phrases::new(),
        };
        let required = match table.get("required") {
            None => false,
            Some(item) => self.required_flag(kind, item)?,
        };
        let icon = match table.get("icon") {
            Some(item) => Some(super::look::icon(self, item)?),
            None => None,
        };
        Ok(Head {
            kind,
            version,
            protocol,
            name,
            summary,
            required,
            icon,
        })
    }

    /// `[package] required`（施工 F-1）：开关，只有内置包能写。
    fn required_flag(&self, kind: PackageKind, item: &Item) -> Result<bool, Problem> {
        if kind != PackageKind::Builtin {
            return Err(self.problem(
                Some(item),
                Code::WrongKind,
                "package.required",
                "package.required is only for kind = \"builtin\"".to_string(),
            ));
        }
        item.as_bool().ok_or_else(|| {
            self.problem(
                Some(item),
                Code::NotBool,
                "package.required",
                "package.required must be true or false".to_string(),
            )
        })
    }

    /// `[command]`。
    pub(super) fn command(&self, table: &dyn TableLike, at: &Item) -> Result<Command, Problem> {
        self.only(table, "command", &["name", "program", "about"])?;
        let item = self.required(table, at, "command", "name")?;
        let line = item.span().map(|span| line_of(self.text, span.start));
        let name = item
            .as_str()
            .filter(|name| command_name(name))
            .ok_or_else(|| {
                self.problem(
                    Some(item),
                    Code::BadCommandName,
                    "command.name",
                    "command.name must start with a lowercase letter and use only lowercase letters, digits and -".to_string(),
                )
            })?;
        let item = self.required(table, at, "command", "program")?;
        let program = item
            .as_str()
            .filter(|program| program_name(program))
            .ok_or_else(|| {
                self.problem(
                    Some(item),
                    Code::BadProgram,
                    "command.program",
                    "command.program must be a program name without a path".to_string(),
                )
            })?;
        let about = self.phrases(
            self.required(table, at, "command", "about")?,
            "command.about",
        )?;
        Ok(Command {
            name: name.to_string(),
            program: program.to_string(),
            about,
            line,
        })
    }

    /// `[process]`。
    pub(super) fn process(&self, table: &dyn TableLike) -> Result<Process, Problem> {
        self.only(
            table,
            "process",
            &["args", "start", "capabilities", "system_account"],
        )?;
        let start = match table.get("start") {
            None => Start::Manual,
            Some(item) => match item.as_str() {
                Some("manual") => Start::Manual,
                Some("always") => Start::Always,
                _ => {
                    return Err(self.problem(
                        Some(item),
                        Code::BadStart,
                        "process.start",
                        "process.start must be manual or always".to_string(),
                    ));
                }
            },
        };
        let system_account = match table.get("system_account") {
            None => false,
            Some(item) => item.as_bool().ok_or_else(|| {
                self.problem(
                    Some(item),
                    Code::NotBool,
                    "process.system_account",
                    "process.system_account must be true or false".to_string(),
                )
            })?,
        };
        Ok(Process {
            args: self.texts(table, "process", "args")?,
            start,
            capabilities: self.capabilities(table)?,
            system_account,
        })
    }

    /// `[process] capabilities`（施工 9-4 下上）：字的列表，每个都是认识的能力名、不重复；照表的先后排好。
    fn capabilities(&self, table: &dyn TableLike) -> Result<Vec<Capability>, Problem> {
        let names = self.texts(table, "process", "capabilities")?;
        let mut capabilities = Vec::new();
        for name in &names {
            match Capability::parse(name) {
                Some(capability) if !capabilities.contains(&capability) => {
                    capabilities.push(capability);
                }
                found => {
                    let why = match found {
                        Some(_) => "is listed twice",
                        None => "is not a known capability",
                    };
                    return Err(self.problem(
                        table.get("capabilities"),
                        Code::BadCapability,
                        name,
                        format!("process.capabilities: \"{name}\" {why}"),
                    ));
                }
            }
        }
        capabilities.sort_unstable();
        Ok(capabilities)
    }

    /// `[ui]`。
    pub(super) fn pages(&self, table: &dyn TableLike) -> Result<Pages, Problem> {
        self.only(table, "ui", &["opens", "pages_dir"])?;
        let opens = match table.get("opens") {
            None => Vec::new(),
            Some(item) => {
                let bad = || {
                    self.problem(
                        Some(item),
                        Code::BadPage,
                        "ui.opens",
                        "ui.opens must list page names in lowercase letters, digits, - and _"
                            .to_string(),
                    )
                };
                let array = item.as_array().ok_or_else(bad)?;
                array
                    .iter()
                    .map(|page| {
                        page.as_str()
                            .filter(|page| page_name(page))
                            .map(str::to_string)
                    })
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(bad)?
            }
        };
        let pages_dir = match table.get("pages_dir") {
            None => None,
            Some(item) => Some(
                item.as_str()
                    .filter(|dir| relative_dir(dir))
                    .map(str::to_string)
                    .ok_or_else(|| {
                        self.problem(
                            Some(item),
                            Code::BadPagesDir,
                            "ui.pages_dir",
                            "ui.pages_dir must be a relative directory inside the resource directory".to_string(),
                        )
                    })?,
            ),
        };
        Ok(Pages { opens, pages_dir })
    }

    /// 这张表给不给 `kind` 这种包（施工 F-1，[`PackageKind::tables`]）。
    pub(super) fn belongs(&self, kind: PackageKind, name: &str, at: &Item) -> Result<(), Problem> {
        if kind.tables().contains(&name) {
            return Ok(());
        }
        Err(self.problem(
            Some(at),
            Code::WrongKind,
            &format!("[{name}]"),
            format!("[{name}] is not for kind = \"{}\"", kind.as_str()),
        ))
    }

    /// 这张表要有 `[command]`。
    pub(super) fn needs_command(
        &self,
        command: Option<&Command>,
        name: &str,
        at: &Item,
    ) -> Result<(), Problem> {
        if command.is_some() {
            return Ok(());
        }
        Err(self.problem(
            Some(at),
            Code::NeedsCommand,
            name,
            format!("[{name}] needs a [command] to name the program"),
        ))
    }

    /// 一格字。
    fn text(&self, item: &Item, key: &str) -> Result<String, Problem> {
        item.as_str().map(str::to_string).ok_or_else(|| {
            self.problem(
                Some(item),
                Code::NotText,
                key,
                format!("{key} must be text"),
            )
        })
    }

    /// 一格字的数组，没写的是空的。
    pub(super) fn texts(
        &self,
        table: &dyn TableLike,
        name: &str,
        key: &str,
    ) -> Result<Vec<String>, Problem> {
        let Some(item) = table.get(key) else {
            return Ok(Vec::new());
        };
        item.as_array()
            .and_then(|array| {
                array
                    .iter()
                    .map(|value| value.as_str().map(str::to_string))
                    .collect::<Option<Vec<_>>>()
            })
            .ok_or_else(|| {
                self.problem(
                    Some(item),
                    Code::NotTexts,
                    &format!("{name}.{key}"),
                    format!("{name}.{key} must be a list of text"),
                )
            })
    }

    /// 一格「语言到一句话」。
    pub(super) fn phrases(&self, item: &Item, key: &str) -> Result<Phrases, Problem> {
        phrases::read(item).map_err(|error| {
            let line = |span: Option<std::ops::Range<usize>>| {
                span.map(|span| line_of(self.text, span.start))
            };
            match error {
                PhraseError::NotPhrases(span) => Problem {
                    line: line(span),
                    code: Code::NotPhrases,
                    detail: key.to_string(),
                    message: format!("{key} must map languages to text"),
                },
                PhraseError::UnknownLanguage(language, span) => Problem {
                    line: line(span),
                    code: Code::UnknownLanguage,
                    detail: format!("{key}.{language}"),
                    message: format!("{key}.{language}: language must be zh, en or ja"),
                },
                PhraseError::Empty(language, span) => Problem {
                    line: line(span),
                    code: Code::EmptyPhrase,
                    detail: format!("{key}.{language}"),
                    message: format!("{key}.{language} must be non-empty text"),
                },
            }
        })
    }
}

/// `[最低, 最高]`：两个非负整数，最低不大于最高。
fn protocol(item: &Item) -> Option<[u32; 2]> {
    let array = item.as_array()?;
    let numbers: Vec<u32> = array
        .iter()
        .map(|value| {
            value
                .as_integer()
                .and_then(|number| u32::try_from(number).ok())
        })
        .collect::<Option<_>>()?;
    match numbers.as_slice() {
        [low, high] if low <= high => Some([*low, *high]),
        _ => None,
    }
}

/// 子命令名：小写字母开头，小写字母、数字、`-`，最多 32 个。
fn command_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && name.len() <= 32
}

/// 程序名：不空，不带 `/`、`\`，不是 `.`、`..`。
pub(super) fn program_name(program: &str) -> bool {
    !program.is_empty() && !program.contains(['/', '\\']) && program != "." && program != ".."
}

/// 页名：小写字母、数字、`-`、`_`，不空。
fn page_name(page: &str) -> bool {
    !page.is_empty()
        && page
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// 相对资源目录的目录：`/` 分隔，每一段不空、不是 `.`、`..`，不以 `/` 开头，不带 `\`、`:`。
pub(super) fn relative_dir(dir: &str) -> bool {
    !dir.is_empty()
        && !dir.contains(['\\', ':'])
        && dir
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

/// 第 `offset` 个字节在第几行（从 1 数）。
pub(super) fn line_of(text: &str, offset: usize) -> usize {
    text.get(..offset)
        .map_or(1, |head| head.matches('\n').count() + 1)
}
