//! `miyu login`、`miyu logout`（施工 8-5，`docs/blueprint/config.md`「怎么走」第十一条、「样子」）：在进程里起一个核心，
//! 在真的套接字上走一遍。人那一头是照剧本回的假终端：在不在终端里、敲什么、贴的 key。
//!
//! 写了名字的直接贴、不写的选（编号、新名字、直接回车、写错的、一个都没有），贴 key 走关掉回显的那一条（假终端记着
//! 走的是哪条）、管道进来的整份读、空的不收；`--list` 两种格式；`logout` 写名字、选、没设过的；两种语言；退出码。屏幕上
//! 从头到尾没有 key。

mod support;

use std::collections::VecDeque;
use std::io;
use std::path::Path;
use std::sync::Arc;

use miyu_cli::language::Language;
use miyu_cli::{Console, Format, KeyCommand, LoginPlan};
use miyu_session::testkit::Script;
use support::{Asked, Home};

/// 一眼看得出是假的 key。
const FAKE: &str = "sk-FAKE-KEY-FOR-TESTS-0001";
const FAKE_2: &str = "sk-FAKE-KEY-FOR-TESTS-0002";

/// 照剧本回的假终端。
#[derive(Default)]
struct Fake {
    /// 标准输入、标准错误都是终端。
    terminal: bool,
    /// 敲的几行（选编号）。
    answers: VecDeque<&'static str>,
    /// 关掉回显读到的、管道里整份的：都是这一份。
    key: String,
    /// 关掉回显读过几次。
    hidden: usize,
    /// 整份读过几次。
    piped: usize,
    /// 关掉回显读的时候，照取消办（按了 `Ctrl+C`），不照 `key`（施工 8-5 补）。
    cancelled: bool,
}

impl Console for Fake {
    fn terminal(&self) -> bool {
        self.terminal
    }

    fn line(&mut self) -> io::Result<Option<String>> {
        Ok(self.answers.pop_front().map(str::to_string))
    }

    fn edit(&mut self, _path: &Path) -> io::Result<Option<i32>> {
        Err(io::Error::other("login 不开编辑器"))
    }

    fn typed(&self) -> bool {
        self.terminal
    }

    fn hidden(&mut self) -> io::Result<Option<String>> {
        self.hidden += 1;
        match self.cancelled {
            true => Err(io::Error::new(io::ErrorKind::Interrupted, "test cancel")),
            false => Ok(Some(self.key.clone())),
        }
    }

    fn all(&mut self) -> io::Result<String> {
        self.piped += 1;
        Ok(self.key.clone())
    }
}

/// 在终端里，敲 `answers`，贴 `key`。
fn typing(answers: &[&'static str], key: &str) -> Fake {
    Fake {
        terminal: true,
        answers: answers.iter().copied().collect(),
        key: key.to_string(),
        ..Fake::default()
    }
}

/// 不在终端里，管道里是 `key`。
fn piping(key: &str) -> Fake {
    Fake {
        key: key.to_string(),
        ..Fake::default()
    }
}

/// 在终端里，关掉回显读的时候照取消办（施工 8-5 补：按了 `Ctrl+C`、或者空行按了 `Ctrl+D`）。
fn cancelling() -> Fake {
    Fake {
        terminal: true,
        cancelled: true,
        ..Fake::default()
    }
}

fn home() -> Home {
    Home::new(Arc::new(Script::new([])))
}

/// 办一次，屏幕上没有 key。
async fn run(home: &Home, language: Language, command: KeyCommand, console: &mut Fake) -> Asked {
    let plan = LoginPlan {
        command,
        language,
        color: false,
        gray: false,
    };
    let asked = home.login(&plan, console).await;
    for key in [FAKE, FAKE_2, "sk-FAKE"] {
        assert!(
            !asked.screen.contains(key),
            "屏幕上有 key：{}",
            asked.screen
        );
    }
    asked
}

fn login(name: &str) -> KeyCommand {
    KeyCommand::Login(Some(name.to_string()))
}

fn logout(name: &str) -> KeyCommand {
    KeyCommand::Logout(Some(name.to_string()))
}

/// 密钥文件现在的字。
fn secrets(home: &Home) -> String {
    std::fs::read_to_string(home.root.path().join("system/secrets.toml")).unwrap_or_default()
}

#[tokio::test]
async fn a_named_login_reads_the_key_hidden_and_says_saved_or_replaced() {
    let home = home();
    let mut console = typing(&[], &format!("{FAKE}\n"));
    let asked = run(&home, Language::Chinese, login("deepseek"), &mut console).await;
    assert_eq!(asked.code, 0, "{}", asked.screen);
    assert_eq!(
        asked.err,
        "粘贴 deepseek 的 key（不显示）：· deepseek 的 key 存好了\n"
    );
    assert_eq!(asked.out, "");
    assert_eq!(
        (console.hidden, console.piped),
        (1, 0),
        "终端里走关掉回显的那一条"
    );
    assert!(secrets(&home).contains(&format!("deepseek = \"{FAKE}\"")));
    let mut console = typing(&[], FAKE_2);
    let asked = run(&home, Language::English, login("deepseek"), &mut console).await;
    assert_eq!(
        asked.err,
        "Paste the key for deepseek (it will not show): · Replaced the key for deepseek\n"
    );
}

/// 贴 key 时取消了（施工 8-5 补：`Console::hidden` 报 [`io::ErrorKind::Interrupted`]，不管是 `Ctrl+C` 还是空行
/// `Ctrl+D`）：说「没存，取消了」，退出码 130，密钥文件一个字都不写。真的终端里按 `Ctrl+C`、终端设置照原样开回回显，
/// 这一条在 `crates/miyu/tests/login_tty.rs` 里用真的伪终端测。
#[tokio::test]
async fn cancelling_the_key_paste_saves_nothing_and_exits_130() {
    let home = home();
    let asked = run(
        &home,
        Language::Chinese,
        login("deepseek"),
        &mut cancelling(),
    )
    .await;
    assert_eq!(
        (asked.code, asked.err.as_str()),
        (130, "粘贴 deepseek 的 key（不显示）：没存，取消了\n")
    );
    assert_eq!(secrets(&home), "", "密钥文件一个字都没写");
    let asked = run(
        &home,
        Language::English,
        login("deepseek"),
        &mut cancelling(),
    )
    .await;
    assert_eq!(
        (asked.code, asked.err.as_str()),
        (
            130,
            "Paste the key for deepseek (it will not show): Not saved, cancelled\n"
        )
    );
}

#[tokio::test]
async fn a_piped_key_is_read_whole_without_asking() {
    let home = home();
    let mut console = piping(&format!("  {FAKE}\n"));
    let asked = run(&home, Language::Chinese, login("deepseek"), &mut console).await;
    assert_eq!(asked.code, 0, "{}", asked.screen);
    assert_eq!(asked.err, "· deepseek 的 key 存好了\n", "管道进来的不问");
    assert_eq!((console.hidden, console.piped), (0, 1));
    assert!(
        secrets(&home).contains(&format!("deepseek = \"{FAKE}\"")),
        "存的去掉了前后空白"
    );
    let asked = run(&home, Language::Chinese, login("empty"), &mut piping(" \n")).await;
    assert_eq!((asked.code, asked.err.as_str()), (1, "没收到 key\n"));
    let asked = run(
        &home,
        Language::Chinese,
        login("odd"),
        &mut piping("sk-FAKE\u{7}"),
    )
    .await;
    assert_eq!(
        (asked.code, asked.err.as_str()),
        (1, "参数不对。\n"),
        "核心拒了照原话说"
    );
}

#[tokio::test]
async fn misuse_is_refused_before_anything() {
    let home = home();
    let asked = run(
        &home,
        Language::Chinese,
        login("Deep Seek"),
        &mut typing(&[], FAKE),
    )
    .await;
    assert_eq!(asked.code, 2);
    assert_eq!(
        asked.err,
        "Deep Seek 不能当 key 的名字：小写字母开头，只有小写字母、数字、-、_，最长 64 个字符\n"
    );
    let asked = run(
        &home,
        Language::Chinese,
        KeyCommand::Login(None),
        &mut piping(FAKE),
    )
    .await;
    assert_eq!(
        (asked.code, asked.err.as_str()),
        (2, "要在终端里选，或者写 miyu login <名字>\n")
    );
    let asked = run(
        &home,
        Language::English,
        KeyCommand::Logout(None),
        &mut piping(""),
    )
    .await;
    assert_eq!(
        (asked.code, asked.err.as_str()),
        (2, "Pick in a terminal, or run miyu logout <name>\n")
    );
    assert_eq!(secrets(&home), "", "什么都没写");
}

#[tokio::test]
async fn picking_takes_a_number_or_a_new_name() {
    let home = home();
    let asked = run(
        &home,
        Language::Chinese,
        KeyCommand::Login(None),
        &mut typing(&[], FAKE),
    )
    .await;
    assert_eq!(
        (asked.code, asked.err.as_str()),
        (2, "配置里还没有用到密钥的供应商：写 miyu login <名字>\n")
    );
    run(
        &home,
        Language::Chinese,
        login("deepseek"),
        &mut piping(FAKE),
    )
    .await;
    run(
        &home,
        Language::Chinese,
        login("bigmodel-2"),
        &mut piping(FAKE),
    )
    .await;
    let mut console = typing(&["2"], FAKE_2);
    let asked = run(
        &home,
        Language::Chinese,
        KeyCommand::Login(None),
        &mut console,
    )
    .await;
    assert_eq!(asked.code, 0, "{}", asked.screen);
    assert_eq!(
        asked.err,
        "配置里用到的密钥：\n  1  bigmodel-2  已设置\n  2  deepseek    已设置\n选一个编号，或者敲一个新名字：粘贴 deepseek 的 key（不显示）：· 换掉了 deepseek 的 key\n"
    );
    let asked = run(
        &home,
        Language::Chinese,
        KeyCommand::Login(None),
        &mut typing(&["zeta"], FAKE),
    )
    .await;
    assert!(
        asked.err.ends_with("· zeta 的 key 存好了\n"),
        "{}",
        asked.err
    );
    let asked = run(
        &home,
        Language::Chinese,
        KeyCommand::Login(None),
        &mut typing(&[""], FAKE),
    )
    .await;
    assert_eq!(asked.code, 1);
    assert!(asked.err.ends_with("没选\n"), "{}", asked.err);
    let asked = run(
        &home,
        Language::Chinese,
        KeyCommand::Login(None),
        &mut typing(&[], FAKE),
    )
    .await;
    assert_eq!(asked.code, 1, "读到头也是没选");
    let asked = run(
        &home,
        Language::Chinese,
        KeyCommand::Login(None),
        &mut typing(&["9"], FAKE),
    )
    .await;
    assert_eq!(asked.code, 2);
    assert!(
        asked.err.ends_with(
            "9 不能当 key 的名字：小写字母开头，只有小写字母、数字、-、_，最长 64 个字符\n"
        ),
        "{}",
        asked.err
    );
}

#[tokio::test]
async fn the_list_never_shows_a_key() {
    let home = home();
    let asked = run(
        &home,
        Language::Chinese,
        KeyCommand::List(Format::Text),
        &mut piping(""),
    )
    .await;
    assert_eq!((asked.code, asked.out.as_str()), (0, "还没有设过 key\n"));
    run(
        &home,
        Language::Chinese,
        login("deepseek"),
        &mut piping(FAKE),
    )
    .await;
    run(
        &home,
        Language::Chinese,
        login("bigmodel-2"),
        &mut piping(FAKE_2),
    )
    .await;
    let asked = run(
        &home,
        Language::Chinese,
        KeyCommand::List(Format::Text),
        &mut piping(""),
    )
    .await;
    assert_eq!(asked.out, "bigmodel-2  已设置\ndeepseek    已设置\n");
    let asked = run(
        &home,
        Language::English,
        KeyCommand::List(Format::Json),
        &mut piping(""),
    )
    .await;
    let listed: serde_json::Value = serde_json::from_str(&asked.out).expect("是 JSON");
    assert_eq!(
        listed,
        serde_json::json!({"secrets": [
            {"name": "bigmodel-2", "set": true, "used_by": []},
            {"name": "deepseek", "set": true, "used_by": []},
        ]})
    );
}

#[tokio::test]
async fn logout_deletes_by_name_or_pick() {
    let home = home();
    let asked = run(
        &home,
        Language::Chinese,
        KeyCommand::Logout(None),
        &mut typing(&["1"], ""),
    )
    .await;
    assert_eq!((asked.code, asked.err.as_str()), (1, "还没有设过 key\n"));
    run(
        &home,
        Language::Chinese,
        login("deepseek"),
        &mut piping(FAKE),
    )
    .await;
    run(
        &home,
        Language::Chinese,
        login("bigmodel-2"),
        &mut piping(FAKE_2),
    )
    .await;
    let asked = run(
        &home,
        Language::Chinese,
        logout("bigmodel-2"),
        &mut piping(""),
    )
    .await;
    assert_eq!(
        (asked.code, asked.err.as_str()),
        (0, "· 删掉了 bigmodel-2 的 key\n")
    );
    let asked = run(
        &home,
        Language::English,
        logout("bigmodel-2"),
        &mut piping(""),
    )
    .await;
    assert_eq!(
        (asked.code, asked.err.as_str()),
        (1, "bigmodel-2 has no key\n")
    );
    let asked = run(
        &home,
        Language::Chinese,
        KeyCommand::Logout(None),
        &mut typing(&["1"], ""),
    )
    .await;
    assert_eq!(asked.code, 0, "{}", asked.screen);
    assert_eq!(
        asked.err,
        "设过的 key：\n  1  deepseek  已设置\n选一个编号：· 删掉了 deepseek 的 key\n"
    );
    assert!(!secrets(&home).contains("sk-FAKE"));
}
