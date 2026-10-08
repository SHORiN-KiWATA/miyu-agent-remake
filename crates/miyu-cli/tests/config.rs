//! `miyu config set`、`unset`、`edit`、`trust`（施工 8-3，`docs/blueprint/config.md`「命令行」第十条、「样子」）：在进程里
//! 起一个核心，在真的套接字上走一遍。人那一头是照剧本回的：在不在终端里、敲什么、编辑器改成什么。
//!
//! 印的那一行照「样子」一字不差；`edit` 改字、不改、改错再改好、放弃、编辑器出错、存的时候别处改过了、项目配置由命令行
//! 自己写；`trust` 列出会改哪几项、`--yes`、`--no`、问、不在终端里、本来就信任着、看的时候又变了。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::support::configuring::{Edit, Fake, at_terminal};
use crate::support::outside::Outside;
use crate::support::{Asked, Home};
use miyu_cli::language::Language;
use miyu_cli::{ConfigCommand, ConfigPlan};
use miyu_session::testkit::Script;

fn home() -> Home {
    Home::new(Arc::new(Script::new([])))
}

/// 在 `cwd` 里办 `command`，中文。
async fn run(home: &Home, cwd: &Path, command: ConfigCommand, console: &mut Fake) -> Asked {
    let plan = ConfigPlan {
        command,
        language: Language::Chinese,
        cwd: cwd.to_path_buf(),
        root: home.root.path().to_path_buf(),
        home: None,
        color: false,
        gray: false,
    };
    home.config(&plan, console).await
}

fn set(key: &str, value: &str, system: bool) -> ConfigCommand {
    ConfigCommand::Set {
        key: key.to_string(),
        value: value.to_string(),
        system,
        project: false,
    }
}

fn unset(key: &str) -> ConfigCommand {
    ConfigCommand::Unset {
        key: key.to_string(),
        system: false,
    }
}

fn edit(project: bool) -> ConfigCommand {
    ConfigCommand::Edit {
        system: false,
        project,
    }
}

fn trust(yes: bool, no: bool) -> ConfigCommand {
    ConfigCommand::Trust { yes, no }
}

/// 管理员的个人设置在哪。
fn personal(home: &Home) -> PathBuf {
    home.root
        .path()
        .join("home")
        .join("admin")
        .join("settings.toml")
}

/// 家目录里还有没有编辑时的副本。
fn copies_left(home: &Home) -> Vec<String> {
    std::fs::read_dir(home.root.path().join("home").join("admin"))
        .map(|entries| {
            entries
                .map(|entry| {
                    entry
                        .expect("在")
                        .file_name()
                        .to_string_lossy()
                        .into_owned()
                })
                .filter(|name| name.contains(".edit-"))
                .collect()
        })
        .unwrap_or_default()
}

#[tokio::test]
async fn set_and_unset_say_what_happened_in_one_gray_line() {
    let home = home();
    let cwd = std::env::temp_dir();
    let mut none = Fake::default();
    let cases = [
        (
            set("ui.language", "zh", false),
            0,
            "· ui.language = \"zh\" 写进了个人设置，当场生效\n",
        ),
        (
            set("ui.language", "\"zh\"", false),
            0,
            "· 本来就是 \"zh\"，没改\n",
        ),
        (
            set("ui.language", "en", true),
            0,
            "· ui.language = \"en\" 写进了系统配置，个人设置里写着 \"zh\"，用的还是 \"zh\"\n",
        ),
        (
            set("permission.start_read_only", "true", false),
            0,
            "· permission.start_read_only = true 写进了个人设置，以后开的会话生效\n",
        ),
        (
            unset("ui.language"),
            0,
            "· 从个人设置里删掉了 ui.language，现在是 \"en\"（系统配置）\n",
        ),
        // 系统配置定了英文：握手以后照英文说。
        (
            unset("ui.language"),
            0,
            "· Personal settings did not have ui.language\n",
        ),
        (
            set("ui.language", "zh", true),
            0,
            "· ui.language = \"zh\" saved to the system config, takes effect at once\n",
        ),
    ];
    for (command, code, said) in cases {
        let asked = run(&home, &cwd, command.clone(), &mut none).await;
        assert_eq!(
            (asked.code, asked.err.as_str()),
            (code, said),
            "{command:?}"
        );
        assert_eq!(asked.out, "");
    }
    let wrong = run(&home, &cwd, set("ui.language", "cn", false), &mut none).await;
    assert_eq!(wrong.code, 1);
    assert_eq!(
        wrong.err,
        "ui.language 只能是 auto、zh、en 或 ja，写的是 cn。改成其中一个，例如 ui.language = \"auto\"。\n"
    );
    let unknown = run(&home, &cwd, set("ui.langauge", "zh", false), &mut none).await;
    assert_eq!(unknown.code, 1);
    assert!(unknown.err.contains("ui.language"), "{}", unknown.err);
    let project = ConfigCommand::Set {
        key: "permission.start_read_only".to_string(),
        value: "true".to_string(),
        system: false,
        project: true,
    };
    let refused = run(&home, &cwd, project, &mut none).await;
    assert_eq!(
        (refused.code, refused.err.as_str()),
        (2, "项目配置只能手改：miyu config edit --project\n")
    );
}

#[tokio::test]
async fn edit_asks_again_until_it_checks_out_and_keeps_what_was_typed() {
    let home = home();
    let cwd = std::env::temp_dir();
    std::fs::create_dir_all(personal(&home).parent().expect("有上一级")).expect("建得了");
    std::fs::write(personal(&home), "[ui]\nlanguage = \"zh\"\n").expect("写得进");
    let wrong = "[ui]\nlanguage = \"cn\"\n";
    let fixed = "# 改好了\n[ui]\nlanguage = \"ja\"\nlangauge = 1\n";
    let mut console = at_terminal(&[""], vec![Edit::Write(wrong), Edit::Write(fixed)]);
    let asked = run(&home, &cwd, edit(false), &mut console).await;
    let shown = personal(&home).to_string_lossy().into_owned();
    assert_eq!(
        asked.err,
        format!(
            "{shown}:2:12 错误：ui.language 只能是 auto、zh、en 或 ja，写的是 \"cn\"。改成其中一个，例如 ui.language = \"auto\"。这一项先照 \"auto\" 用着（默认值）。\n\
             有 1 处错误，还没存。回车接着改，输入 q 放弃：\
             {shown}:4:1 警告：没有 ui.langauge 这一项。是不是想写 ui.language？这一行先不管，原样留着。\n\
             · 存好了，当场生效\n"
        )
    );
    assert_eq!(asked.code, 0);
    assert_eq!(
        std::fs::read_to_string(personal(&home)).expect("在"),
        fixed,
        "只有警告的照样存"
    );
    let [(first, before), (again, kept)] = console.opened.as_slice() else {
        panic!("开了两次：{:?}", console.opened);
    };
    assert_eq!(before, "[ui]\nlanguage = \"zh\"\n", "副本是现在的字");
    assert_eq!(
        (again, kept.as_str()),
        (first, wrong),
        "回车再打开，改的还在"
    );
    assert_eq!(first.parent(), personal(&home).parent(), "副本在原文件旁边");
    let name = first
        .file_name()
        .expect("有名字")
        .to_string_lossy()
        .into_owned();
    assert!(
        name.starts_with(".settings.toml.edit-") && name.ends_with(".toml"),
        "{name}"
    );
    assert_eq!(copies_left(&home), Vec::<String>::new(), "存好了删掉副本");
}

#[tokio::test]
async fn edit_leaves_the_file_alone_when_nothing_is_saved() {
    let home = home();
    let cwd = std::env::temp_dir();
    let bad = "[ui]\nlanguage = \"cn\"\n";
    let cases: [(Fake, u8, &str); 4] = [
        (at_terminal(&[], vec![Edit::Keep]), 0, "没改\n"),
        (
            at_terminal(&["q"], vec![Edit::Write(bad)]),
            1,
            "放弃了，文件没动\n",
        ),
        (
            at_terminal(&[], vec![Edit::Write(bad)]),
            1,
            "放弃了，文件没动\n",
        ),
        (
            at_terminal(&[], vec![Edit::Fail(3)]),
            1,
            "编辑器没有正常退出（3），没存\n",
        ),
    ];
    for (mut console, code, ends) in cases {
        let asked = run(&home, &cwd, edit(false), &mut console).await;
        assert_eq!(asked.code, code, "{}", asked.err);
        assert!(asked.err.ends_with(ends), "{}", asked.err);
        assert!(!personal(&home).exists(), "文件没动");
        // 核心起来时会在账号目录下建会话列表的索引（施工 3-8 七补），那一份不算编辑留下的。
        let account = personal(&home).parent().expect("有上一级").to_path_buf();
        let left: Vec<String> = std::fs::read_dir(&account)
            .map(|entries| {
                entries
                    .map(|entry| {
                        entry
                            .expect("读得了")
                            .file_name()
                            .to_string_lossy()
                            .into_owned()
                    })
                    .filter(|name| name != "index")
                    .collect()
            })
            .unwrap_or_default();
        assert!(left.is_empty(), "为副本新建的东西也删掉：{left:?}");
    }
    let mut elsewhere = Fake::default();
    let asked = run(&home, &cwd, edit(false), &mut elsewhere).await;
    assert_eq!(
        (asked.code, asked.err.as_str()),
        (2, "miyu config edit 要在终端里用\n"),
        "不在终端里"
    );
}

#[tokio::test]
async fn edit_keeps_the_copy_when_the_file_changed_meanwhile() {
    let home = home();
    let cwd = std::env::temp_dir();
    std::fs::create_dir_all(personal(&home).parent().expect("有上一级")).expect("建得了");
    std::fs::write(personal(&home), "[ui]\nlanguage = \"zh\"\n").expect("写得进");
    let mine = "[ui]\nlanguage = \"ja\"\n";
    let theirs = "[ui]\nlanguage = \"zh\"\n# 别处改的\n";
    let mut console = at_terminal(&[], vec![Edit::Meanwhile(personal(&home), theirs, mine)]);
    let asked = run(&home, &cwd, edit(false), &mut console).await;
    let copy = &console.opened[0].0;
    assert_eq!(asked.code, 1);
    assert_eq!(
        asked.err,
        format!(
            "你编辑的时候文件被改过了，没存。你改的在 {}\n",
            copy.to_string_lossy()
        )
    );
    assert_eq!(
        std::fs::read_to_string(personal(&home)).expect("在"),
        theirs
    );
    assert_eq!(std::fs::read_to_string(copy).expect("副本留着"), mine);
}

/// 一个仓库（有 `.git`），交回它真实的位置。
fn repository(outside: &Outside) -> PathBuf {
    let repo = outside.0.join("app");
    std::fs::create_dir_all(repo.join(".git")).expect("建得了");
    std::fs::create_dir_all(repo.join("src")).expect("建得了");
    std::fs::canonicalize(&repo).expect("在")
}

/// 核心报的项目配置的写法：真实的位置，Windows 上去掉开头的 `\\?\`。
fn shown(path: &Path) -> String {
    let text = path.to_string_lossy();
    text.strip_prefix(r"\\?\").unwrap_or(&text).to_string()
}

#[tokio::test]
async fn a_project_config_is_edited_and_saved_by_the_command_line_then_trusted() {
    let home = home();
    let outside = Outside::new();
    let repo = repository(&outside);
    let cwd = repo.join("src");
    let none = run(&home, &cwd, trust(false, false), &mut Fake::default()).await;
    assert_eq!((none.code, none.err.as_str()), (1, "这里没有项目配置\n"));

    let written = "[permission]\nstart_read_only = true\n\n[ui]\nlanguage = \"zh\"\n";
    let mut console = at_terminal(
        &[""],
        vec![
            Edit::Write(written),
            Edit::Write("[permission]\nstart_read_only = true\n"),
        ],
    );
    let edited = run(&home, &cwd, edit(true), &mut console).await;
    assert_eq!(edited.code, 0, "{}", edited.err);
    assert!(
        edited
            .err
            .contains("ui.language 只能写在系统配置或个人设置里"),
        "{}",
        edited.err
    );
    assert!(edited.err.ends_with("· 存好了\n"), "{}", edited.err);
    let file = repo.join(".miyu").join("config.toml");
    assert_eq!(
        std::fs::read_to_string(&file).expect("在仓库的根新建了"),
        "[permission]\nstart_read_only = true\n"
    );

    let listed = run(&home, &cwd, trust(false, false), &mut Fake::default()).await;
    let header = format!(
        "{} 会改这几项：\n  permission.start_read_only = true  新会话开局只读\n",
        shown(&file)
    );
    assert_eq!(listed.out, header);
    assert_eq!(
        (listed.code, listed.err.as_str()),
        (2, "要在终端里回答，或者写 --yes、--no\n"),
        "不在终端里又没写 --yes、--no"
    );
    let asked = run(
        &home,
        &cwd,
        trust(false, false),
        &mut at_terminal(&["Yes"], Vec::new()),
    )
    .await;
    assert_eq!(asked.out, header);
    assert_eq!(
        (asked.code, asked.err.as_str()),
        (
            0,
            "项目配置只能让限制更严。信任这一份吗？[y/N] · 信任了。内容变了会再问\n"
        )
    );
    let again = run(&home, &cwd, trust(false, false), &mut Fake::default()).await;
    assert_eq!(
        (again.code, again.err.as_str()),
        (0, "已经信任过这一份了\n")
    );
    let no = run(&home, &cwd, trust(false, true), &mut Fake::default()).await;
    assert_eq!(
        (no.code, no.err.as_str()),
        (0, "· 不信任，这一份不会用。内容变了会再问\n")
    );
    let enter = run(
        &home,
        &cwd,
        trust(false, false),
        &mut at_terminal(&[""], Vec::new()),
    )
    .await;
    assert!(
        enter
            .err
            .ends_with("· 不信任，这一份不会用。内容变了会再问\n"),
        "直接回车是不信任"
    );
    let yes = run(&home, &cwd, trust(true, false), &mut Fake::default()).await;
    assert_eq!(
        (yes.code, yes.err.as_str()),
        (0, "· 信任了。内容变了会再问\n")
    );
}

#[tokio::test]
async fn trust_lists_problems_and_refuses_a_version_that_changed_while_looking() {
    let home = home();
    let outside = Outside::new();
    let repo = repository(&outside);
    let file = repo.join(".miyu").join("config.toml");
    std::fs::create_dir_all(file.parent().expect("有上一级")).expect("建得了");
    std::fs::write(
        &file,
        "[permission]\nstart_read_only = true\n[ui]\nlanguage = \"zh\"\n",
    )
    .expect("写得进");
    let mut console = at_terminal(&["y"], Vec::new());
    console.meanwhile = Some((file.clone(), "[permission]\nstart_read_only = true\n"));
    let asked = run(&home, &repo, trust(false, false), &mut console).await;
    assert_eq!(
        asked.out,
        format!(
            "{0} 会改这几项：\n  permission.start_read_only = true  新会话开局只读\n\
             {0}:4:12 错误：ui.language 只能写在系统配置或个人设置里，写在项目配置里不算。挪到系统配置或个人设置里去。\n",
            shown(&file)
        ),
        "不能写的那一项不列，照报错一行印在后面"
    );
    assert_eq!(asked.code, 1);
    assert!(
        asked.err.ends_with("你看的时候它又被改过了，再跑一次\n"),
        "{}",
        asked.err
    );
    assert!(
        !home
            .root
            .path()
            .join("home")
            .join("admin")
            .join("trust.toml")
            .exists()
    );
}
