//! `miyu undo`、`miyu redo`（`docs/construction/4-7-miyu undo、miyu redo（下）.md`）：在进程里起一个核心，工具是真的；
//! 她用 `miyu ask` 改了一个文件，`miyu undo` 改回来、照定的样子印在标准输出上，`miyu restore` 又改回她改完的样子；
//! 之后又被改过的印出差异；一轮都没有的、一个会话都没有的，说清楚，退出码 1。

mod support;

use std::sync::Arc;

use serde_json::json;

use miyu_cli::language::Language;
use miyu_cli::{Direction, Plan, UndoPlan};
use miyu_session::testkit::{Play, Script};
use support::{Home, Outside, plan, resources};

/// 起一个核心：请求模型照 `plays`，工具是出厂的那几件。
fn home(plays: impl IntoIterator<Item = Play>) -> Home {
    let tools = miyu_core::tools(&resources()).expect("出厂的资源读得出来");
    Home::with_tools(Arc::new(Script::new(plays)), tools)
}

/// 读 `a.txt`，再写成 `new`。
fn edit() -> Vec<Play> {
    vec![
        Play::calls(&[("read", &json!({"file_path": "a.txt"}).to_string())]),
        Play::calls(&[(
            "write",
            &json!({"file_path": "a.txt", "content": "new\n"}).to_string(),
        )]),
        Play::Says("改好了。"),
    ]
}

/// 撤最新的那个一次性会话（恢复），中文，不上色。
fn undo(direction: Direction) -> UndoPlan {
    UndoPlan {
        direction,
        session: None,
        language: Language::Chinese,
        home: None,
        color: false,
    }
}

/// 在 `work` 里用 `miyu ask` 说「改一下」，她把 `a.txt` 从 `old` 改成 `new`。
async fn edited(home: &Home, work: &Outside) {
    let asked = home
        .ask(&Plan {
            cwd: work.text(),
            ..plan("改一下")
        })
        .await;
    assert_eq!(asked.code, 0, "{}", asked.err);
}

#[tokio::test]
async fn undo_puts_the_file_back_and_says_so_and_restore_brings_it_again() {
    let work = Outside::new();
    let file = work.file("a.txt", "old\n");
    let home = home(edit());
    edited(&home, &work).await;
    assert_eq!(std::fs::read_to_string(&file).expect("在"), "new\n");
    let undone = home.undo(&undo(Direction::Undo)).await;
    assert_eq!(undone.code, 0, "{}", undone.err);
    assert_eq!(
        undone.out,
        "· 撤销「改一下」这一轮\n· 改回 a.txt\n发下一句之前，可以用 miyu restore 恢复。\n"
    );
    assert_eq!(undone.err, "", "结果走标准输出");
    assert_eq!(std::fs::read_to_string(&file).expect("在"), "old\n");
    let restored = home.undo(&undo(Direction::Restore)).await;
    assert_eq!(restored.code, 0, "{}", restored.err);
    assert_eq!(restored.out, "· 恢复「改一下」这一轮\n· 改回 a.txt\n");
    assert_eq!(std::fs::read_to_string(&file).expect("在"), "new\n");
}

#[tokio::test]
async fn a_file_changed_since_is_shown_with_its_diff() {
    let work = Outside::new();
    let file = work.file("a.txt", "old\n");
    let home = home(edit());
    edited(&home, &work).await;
    std::fs::write(&file, "someone\n").expect("写得进");
    let undone = home.undo(&undo(Direction::Undo)).await;
    assert_eq!(undone.code, 0, "有文件没动也是 0：{}", undone.err);
    assert_eq!(
        undone.out,
        "· 撤销「改一下」这一轮\n· 改回 a.txt → 没动：之后又被改过\n    --- 她改完的\n    +++ 现在\n    @@ -1 +1 @@\n    -new\n    +someone\n发下一句之前，可以用 miyu restore 恢复。\n"
    );
    assert_eq!(std::fs::read_to_string(&file).expect("在"), "someone\n");
}

#[tokio::test]
async fn with_nothing_left_to_undo_it_says_so() {
    let work = Outside::new();
    work.file("a.txt", "old\n");
    let home = home(edit());
    edited(&home, &work).await;
    assert_eq!(home.undo(&undo(Direction::Undo)).await.code, 0);
    let again = home.undo(&undo(Direction::Undo)).await;
    assert_eq!(again.code, 1);
    assert_eq!(again.out, "");
    assert_eq!(again.err, "没有能撤销的回合。\n");
}

#[tokio::test]
async fn without_a_session_it_says_so() {
    let home = home([]);
    let undone = home.undo(&undo(Direction::Undo)).await;
    assert_eq!(undone.code, 1);
    assert!(
        undone.err.contains("还没有 miyu ask 开过的会话"),
        "{}",
        undone.err
    );
}

/// `--session` 撤的是指定的那个会话，不是最新的那个一次性会话。
#[tokio::test]
async fn a_given_session_is_the_one_undone() {
    let work = Outside::new();
    let file = work.file("a.txt", "old\n");
    let mut plays = edit();
    plays.push(Play::Says("在。"));
    let home = home(plays);
    edited(&home, &work).await;
    let later = home
        .ask(&Plan {
            cwd: work.text(),
            ..plan("在吗")
        })
        .await;
    assert_eq!(later.code, 0, "{}", later.err);
    // 从新到旧：最新的是后说的那一句，改了文件的是前一个。
    let first = home.sessions()[1].to_string();
    let undone = home
        .undo(&UndoPlan {
            session: Some(first),
            ..undo(Direction::Undo)
        })
        .await;
    assert!(
        undone
            .out
            .starts_with("· 撤销「改一下」这一轮\n· 改回 a.txt\n"),
        "{}",
        undone.out
    );
    assert_eq!(std::fs::read_to_string(&file).expect("在"), "old\n");
}
