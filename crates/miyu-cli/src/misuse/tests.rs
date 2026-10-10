//! 参数写错时说的那一句（施工 4-11）：每一种错、两种语言；值怎么连；控制字符换掉；必写的只有一个。

use clap::{Args, Command};

use super::{listed, misuse};
use crate::language::Language;
use crate::{Ask, Sandbox, Undo};

/// 和主程序一样的几条子命令。
fn miyu() -> Command {
    Command::new("miyu")
        .subcommand(Ask::augment_args(Command::new("ask")))
        .subcommand(Undo::augment_args(Command::new("undo")))
        .subcommand(Sandbox::augment_args(Command::new("sandbox")))
}

/// 敲 `miyu <args>`：clap 报的错说成的那一句。
fn said(args: &[&str], language: Language) -> String {
    let error = miyu()
        .try_get_matches_from(std::iter::once("miyu").chain(args.iter().copied()))
        .expect_err("这样敲是错的");
    misuse(&error, language)
}

#[test]
fn each_kind_of_mistake_has_one_sentence() {
    let cases: [(&[&str], &str, &str); 9] = [
        (
            &["ask"],
            "少了要说的话：miyu ask \"…\"",
            "Missing what to say: miyu ask \"…\"",
        ),
        (
            &["ask", "--bogus", "hi"],
            "没有 --bogus 这个选项",
            "No such option: --bogus",
        ),
        (
            &["ask", "-x", "hi"],
            "没有 -x 这个选项",
            "No such option: -x",
        ),
        (
            &["--bogus"],
            "没有 --bogus 这个选项",
            "No such option: --bogus",
        ),
        (
            &["undo", "extra"],
            "多了参数：extra",
            "Unexpected argument: extra",
        ),
        (
            &["ask", "-s", "x", "-c", "hi"],
            "--session 和 --continue 只能写一个",
            "--session and --continue can't be used together",
        ),
        (
            &["undo", "--session"],
            "--session 后面少了值",
            "--session needs a value",
        ),
        (
            &["ask", "--format"],
            "--format 后面少了值",
            "--format needs a value",
        ),
        (
            &["ask", "--format", "xml", "hi"],
            "--format 只能是 text 或 json",
            "--format must be text or json",
        ),
    ];
    for (args, chinese, english) in cases {
        assert_eq!(said(args, Language::Chinese), chinese, "{args:?}");
        assert_eq!(said(args, Language::English), english, "{args:?}");
    }
    // 不认识的子命令照 22 第二节的那一句。
    assert_eq!(
        said(&["hello"], Language::Chinese),
        "没有 hello 这个子命令。想和 AI 对话，用 miyu ask \"…\""
    );
}

#[test]
fn anything_else_says_what_clap_said_on_its_first_line() {
    assert_eq!(
        said(&["ask", "--continue=yes", "hi"], Language::Chinese),
        "参数不对：unexpected value 'yes' for '--continue' found; no more were expected"
    );
    assert_eq!(
        said(&["ask", "--continue=yes", "hi"], Language::English),
        "Bad arguments: unexpected value 'yes' for '--continue' found; no more were expected"
    );
}

#[test]
fn control_characters_typed_in_are_replaced() {
    assert_eq!(
        said(&["ask", "--bo\x1b[2Jgus", "hi"], Language::Chinese),
        "没有 --bo\u{FFFD}[2Jgus 这个选项"
    );
    assert_eq!(
        said(&["undo", "a\x07b"], Language::English),
        "Unexpected argument: a\u{FFFD}b"
    );
}

#[test]
fn values_are_joined_like_a_sentence() {
    let values = |names: &[&str]| names.iter().map(ToString::to_string).collect::<Vec<_>>();
    assert_eq!(listed(&values(&[]), "、", " 或 "), "");
    assert_eq!(listed(&values(&["a"]), "、", " 或 "), "a");
    assert_eq!(listed(&values(&["a", "b"]), "、", " 或 "), "a 或 b");
    assert_eq!(listed(&values(&["a", "b", "c"]), "、", " 或 "), "a、b 或 c");
    assert_eq!(listed(&values(&["a", "b", "c"]), ", ", " or "), "a, b or c");
}

#[test]
fn only_what_to_say_is_required() {
    // 必写的多了一个，「少了要说的话」那一句就说错了：这里红了，`misuse` 跟着改。
    let required: Vec<String> = miyu()
        .get_subcommands()
        .flat_map(|command| {
            command
                .get_arguments()
                .filter(|arg| arg.is_required_set())
                .map(|arg| format!("{} {}", command.get_name(), arg.get_id()))
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(required, ["ask words"]);
}

#[test]
fn nested_commands_have_their_own_sentences() {
    // `miyu sandbox` 是第一条嵌着子命令的（施工 5-8）：少了、写错了，说的是它那一层，不提 `miyu ask`。
    let cases: [(&[&str], &str, &str); 4] = [
        (
            &["sandbox"],
            "miyu sandbox 后面要写：setup 或 remove",
            "miyu sandbox needs one of: setup or remove",
        ),
        (
            &["sandbox", "frob"],
            "miyu sandbox 没有 frob 这个子命令",
            "miyu sandbox has no frob command",
        ),
        (
            &["sandbox", "setup", "--owner-home", "/x"],
            "少了 --owner-sid",
            "Missing --owner-sid",
        ),
        (
            &["sandbox", "remove", "extra"],
            "多了参数：extra",
            "Unexpected argument: extra",
        ),
    ];
    for (args, chinese, english) in cases {
        assert_eq!(said(args, Language::Chinese), chinese, "{args:?}");
        assert_eq!(said(args, Language::English), english, "{args:?}");
    }
    // 最外面那一层写错的，照旧那一句。
    assert_eq!(
        said(&["sandboxx"], Language::English),
        "There is no sandboxx command. To talk to the AI, use miyu ask \"…\""
    );
}

/// `--add-dir` 后面不是一个已经有的目录（施工 5-10 上）：照写的原样说是哪一个。
#[test]
fn an_added_dir_that_is_not_there_has_its_sentence() {
    let args = ["ask", "--add-dir", "no-such-dir-for-miyu-cli-tests", "hi"];
    assert_eq!(
        said(&args, Language::Chinese),
        "--add-dir 后面要写一个已经有的目录：no-such-dir-for-miyu-cli-tests"
    );
    assert_eq!(
        said(&args, Language::English),
        "--add-dir needs an existing directory: no-such-dir-for-miyu-cli-tests"
    );
}

/// `--timeout` 后面不是一个时长（施工 7-9）：照写的原样说是哪一个，举几个能写的样子。
#[test]
fn a_timeout_that_is_not_a_duration_has_its_sentence() {
    let args = ["ask", "--timeout", "5min", "hi"];
    assert_eq!(
        said(&args, Language::Chinese),
        "--timeout 后面要写一个时长，例如 30s、10m、1h：5min"
    );
    assert_eq!(
        said(&args, Language::English),
        "--timeout needs a duration such as 30s, 10m or 1h: 5min"
    );
}

/// `--from` 后面是空的、只有空白（施工 7-10）：说要写别的 harness 的名字；后面什么都没写的照「少了值」说。
#[test]
fn a_blank_from_has_its_sentence() {
    for value in ["", "  "] {
        let args = ["ask", "--from", value, "hi"];
        assert_eq!(
            said(&args, Language::Chinese),
            "--from 后面要写别的 harness 的名字"
        );
        assert_eq!(
            said(&args, Language::English),
            "--from needs the name of the other harness"
        );
    }
    assert_eq!(
        said(&["ask", "hi", "--from"], Language::Chinese),
        "--from 后面少了值"
    );
}
