//! 输入法跟着打字状态切（蓝图「配置页」第 30 条，2026-10-07 项目主人：回车进编辑、改完出来还要手动切英文）。离开打字（编辑窗里
//! 改完一项、筛完、关掉）时问一下输入法开没开，开着的关成英文、记住；再进打字时开回来。和 fcitx.vim 一个做法。命令照
//! `layout.json` 的 `ime`，在单独的线程里跑，不卡画面；没装、出错的什么都不做。只在环境变量看得出用的是 fcitx 时切，
//! `MIYU_TUI_IME=0` 关掉（测试、测具都关：不然会切走跑测试那台机器上人正在用的输入法）。

use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Sender};

use serde::Deserialize;

/// 关掉的环境变量。
const OFF: &str = "MIYU_TUI_IME";

/// 怎么问、怎么切（`layout.json` 的 `ime`）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Look {
    /// 看这几个环境变量里有没有 `word`，有才切。
    pub detect_env: Vec<String>,
    /// 认哪个词。
    pub word: String,
    /// 问开没开的命令。
    pub query: Vec<String>,
    /// 问的命令印出这个就是开着。
    pub active: String,
    /// 关成英文的命令。
    pub off: Vec<String>,
    /// 开回来的命令。
    pub on: Vec<String>,
}

/// 管输入法的那一头：界面告诉它进没进打字状态。
#[derive(Debug, Default)]
pub struct Ime {
    tell: Option<Sender<bool>>,
    typing: Option<bool>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl Ime {
    /// 看环境，要切的起一个线程；不切的什么都不做。
    pub fn start(look: &Look, env: impl Fn(&str) -> Option<String>) -> Self {
        // 单元测试里不切：跑测试那台机器上的输入法是人在用的。
        if cfg!(test) || !wanted(look, &env) {
            return Self::default();
        }
        let (tell, heard) = mpsc::channel::<bool>();
        let look = look.clone();
        let worker = std::thread::spawn(move || {
            let mut was_on = false;
            for typing in heard {
                was_on = step(typing, was_on, || query(&look), run, &look);
            }
        });
        Self {
            tell: Some(tell),
            typing: None,
            worker: Some(worker),
        }
    }

    /// 现在是不是在打字；变了才去切。
    pub fn typing(&mut self, typing: bool) {
        if self.typing == Some(typing) {
            return;
        }
        let first = self.typing.is_none();
        self.typing = Some(typing);
        // 一起来就在打字（对话的输入框）：不动人原来的输入法。
        if first && typing {
            return;
        }
        if let Some(tell) = &self.tell
            && tell.send(typing).is_err()
        {
            self.tell = None;
        }
    }
}

impl Ime {
    /// 退出程序前：关掉过的开回来，等它切完（不然进程一退线程就没了）。
    pub fn finish(mut self) {
        self.typing(true);
        drop(self.tell.take());
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {}
    }
}

/// 环境变量看得出用的是 fcitx、没被关掉。
fn wanted(look: &Look, env: &impl Fn(&str) -> Option<String>) -> bool {
    if env(OFF).as_deref() == Some("0") || look.query.is_empty() {
        return false;
    }
    look.detect_env
        .iter()
        .any(|name| env(name).is_some_and(|v| v.contains(&look.word)))
}

/// 切一次：离开打字时开着的关掉、记住；进打字时记着的开回来。交回现在记着没有。
fn step(
    typing: bool,
    was_on: bool,
    query: impl Fn() -> bool,
    run: impl Fn(&[String]),
    look: &Look,
) -> bool {
    if typing {
        if was_on {
            run(&look.on);
        }
        false
    } else if query() {
        run(&look.off);
        true
    } else {
        was_on
    }
}

fn query(look: &Look) -> bool {
    let Some((program, args)) = look.query.split_first() else {
        return false;
    };
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .is_ok_and(|out| String::from_utf8_lossy(&out.stdout).trim() == look.active)
}

fn run(cmd: &[String]) {
    if let Some((program, args)) = cmd.split_first() {
        // 切不成（没装、守护进程没起）不管：下次照样试。
        let status = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if status.is_err() {}
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::{Look, step, wanted};

    fn look() -> Look {
        let words = |list: &[&str]| list.iter().map(|s| s.to_string()).collect();
        Look {
            detect_env: words(&["XMODIFIERS"]),
            word: "fcitx".into(),
            query: words(&["q"]),
            active: "2".into(),
            off: words(&["off"]),
            on: words(&["on"]),
        }
    }

    #[test]
    fn leaving_typing_turns_an_active_ime_off_and_coming_back_restores_it() {
        let look = look();
        let ran = RefCell::new(Vec::new());
        let run = |cmd: &[String]| ran.borrow_mut().push(cmd[0].clone());
        let was = step(false, false, || true, run, &look);
        assert!(was);
        let was = step(true, was, || true, run, &look);
        assert!(!was);
        assert_eq!(*ran.borrow(), ["off", "on"]);
        ran.borrow_mut().clear();
        let was = step(false, false, || false, run, &look);
        let _ = step(true, was, || false, run, &look);
        assert!(ran.borrow().is_empty(), "原来就是英文的不碰");
    }

    #[test]
    fn only_when_the_environment_says_fcitx_and_not_switched_off() {
        let look = look();
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| {
                pairs
                    .iter()
                    .find(|(k, _)| *k == name)
                    .map(|(_, v)| v.to_string())
            }
        };
        assert!(wanted(&look, &env(&[("XMODIFIERS", "@im=fcitx")])));
        assert!(!wanted(&look, &env(&[("XMODIFIERS", "@im=ibus")])));
        assert!(!wanted(
            &look,
            &env(&[("XMODIFIERS", "@im=fcitx"), ("MIYU_TUI_IME", "0")])
        ));
    }
}
