//! 在进程里的核心上办一次 `miyu config`（施工 8-3）：人那一头照测试给的剧本回（施工 8-5 从 `config.rs` 挪来：那个文件
//! 超过 500 行）。

use std::collections::VecDeque;
use std::io;
use std::path::{Path, PathBuf};

use miyu_cli::{ConfigPlan, Console, config_on};

use super::{Asked, Home, Tape, within};

impl Home {
    /// 在真的套接字上连上核心，照 `plan` 办一次 `miyu config`，人那一头是 `console`。
    pub async fn config(&self, plan: &ConfigPlan, console: &mut dyn Console) -> Asked {
        let (connection, token) = miyu_ipc::connect(&self.root).await.expect("连得上");
        let tape = Tape::default();
        let (mut out, mut err) = (tape.pen(false), tape.pen(true));
        let code = within(
            "办完",
            config_on(connection, &token, plan, console, &mut out, &mut err),
        )
        .await;
        Asked {
            code,
            out: tape.text(|err| !err),
            err: tape.text(|err| err),
            screen: tape.text(|_| true),
        }
    }
}

/// 编辑器这一次怎么改。
pub enum Edit {
    /// 副本改成这些字。
    Write(&'static str),
    /// 不动。
    Keep,
    /// 退出码不是 0。
    Fail(i32),
    /// 改副本的同时，别处把真的文件改成了另一份。
    Meanwhile(PathBuf, &'static str, &'static str),
}

/// 照剧本回的人那一头。
#[derive(Default)]
pub struct Fake {
    pub terminal: bool,
    pub answers: VecDeque<&'static str>,
    pub edits: VecDeque<Edit>,
    /// 问人的时候，别处把这份文件改成这些字（`trust` 看的时候又变了）。
    pub meanwhile: Option<(PathBuf, &'static str)>,
    /// 编辑器打开过的副本，和打开时里面的字。
    pub opened: Vec<(PathBuf, String)>,
}

impl Console for Fake {
    fn terminal(&self) -> bool {
        self.terminal
    }

    fn line(&mut self) -> io::Result<Option<String>> {
        if let Some((path, text)) = self.meanwhile.take() {
            std::fs::write(path, text)?;
        }
        Ok(self.answers.pop_front().map(str::to_string))
    }

    fn edit(&mut self, path: &Path) -> io::Result<Option<i32>> {
        self.opened
            .push((path.to_path_buf(), std::fs::read_to_string(path)?));
        match self.edits.pop_front() {
            Some(Edit::Write(text)) => std::fs::write(path, text).map(|()| Some(0)),
            Some(Edit::Keep) | None => Ok(Some(0)),
            Some(Edit::Fail(code)) => Ok(Some(code)),
            Some(Edit::Meanwhile(real, other, text)) => {
                std::fs::write(real, other)?;
                std::fs::write(path, text).map(|()| Some(0))
            }
        }
    }

    fn typed(&self) -> bool {
        self.terminal
    }

    fn hidden(&mut self) -> io::Result<Option<String>> {
        Err(io::Error::other("config 不读 key"))
    }

    fn all(&mut self) -> io::Result<String> {
        Err(io::Error::other("config 不读管道"))
    }
}

/// 在终端里、敲 `answers`、编辑器照 `edits` 改。
pub fn at_terminal(answers: &[&'static str], edits: Vec<Edit>) -> Fake {
    Fake {
        terminal: true,
        answers: answers.iter().copied().collect(),
        edits: edits.into(),
        ..Fake::default()
    }
}
