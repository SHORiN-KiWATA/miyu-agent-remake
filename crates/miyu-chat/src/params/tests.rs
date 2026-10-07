//! 读出厂文件（`chat.md` 第八条「守着它的」）：仓库里的 `defaults.toml` 读得出、零问题、每一项的值对；缺一项、多一项、
//! 多一张表、表写成别的、类型不对、超出范围、`nan`、`inf`、`restraint_k = 0`、时长 `0s` 各报对的问题；拼错的给最近的
//! 名字；`judge.model` 可以不写；写错的不再报缺；有问题整份不用；问题的先后。拿到的 `Chatty`、`Outbound` 和测试里手写的
//! 一样，照 18 第七节的两个例子算分也一样。

use miyu_config::problem::{At, Code};

use crate::chatty::test_support::{self as chatty_support, OTHER, cents, facts, hits, mills};
use crate::judge::test_support::REASON_CHARS;
use crate::outbound::test_support as outbound_support;
use crate::rules::{Problem, Source};
use crate::{Base64, Chatty, Judge, Judgement, Params, score};

use super::test_support::{DEFAULTS, NAME, defaults, file};

/// 仓库里的出厂文件改一处：`from` 正好出现一次，换成 `to`。
fn edited(from: &str, to: &str) -> String {
    assert_eq!(DEFAULTS.matches(from).count(), 1, "{from} 要正好出现一次");
    DEFAULTS.replace(from, to)
}

/// 一条问题看的几样：原因码、键、位置、收到的、最近的名字。
type Seen = (
    Code,
    Option<String>,
    Option<At>,
    Option<String>,
    Option<String>,
);

/// 读 `text`，断定有问题，交回每一条看的几样；顺带断定每一条都照交进来的文件、没有第几条规则。
fn problems(text: &str) -> Vec<Seen> {
    let problems: Vec<Problem> = Params::read(&file(text)).expect_err("应当有问题");
    for problem in &problems {
        assert_eq!(problem.source, Source::Factory);
        assert_eq!(problem.file, NAME);
        assert_eq!(problem.rule, None);
    }
    problems
        .into_iter()
        .map(|p| (p.code, p.key, p.at, p.got, p.suggest))
        .collect()
}

/// 一项的问题：值写错了，指到值。
fn bad(code: Code, key: &str, line: usize, column: usize, got: &str) -> Seen {
    (
        code,
        Some(key.into()),
        Some(At { line, column }),
        Some(got.into()),
        None,
    )
}

/// 缺了的一项：没有位置、没有收到的。
fn missing(key: &str) -> Seen {
    (Code::WrongType, Some(key.into()), None, None, None)
}

/// 出厂文件里 `needle` 在第几行，从 1 数。
fn line_of(text: &str, needle: &str) -> usize {
    text.lines()
        .position(|line| line.starts_with(needle))
        .map(|index| index + 1)
        .expect(needle)
}

/// 读出厂文件改过的一份里某一项的原因码：断定只有这一条问题，键是 `key`。
fn code_of(text: &str, key: &str) -> Code {
    let seen = problems(text);
    assert_eq!(seen.len(), 1, "{seen:?}");
    assert_eq!(seen[0].1.as_deref(), Some(key));
    seen[0].0
}

/// 照仓库里的出厂文件该读出来的：旧版的默认值。`Chatty`、`Outbound` 照测试里手写的那一份（`chatty`、`outbound` 的
/// `test_support`），抽样那一格手写的是 0（别的测试不让抽样掺进来），出厂是 50。
fn factory() -> Params {
    Params {
        base64: Base64 {
            min_chars: 24,
            max_chars: 5000,
            printable: 850,
        },
        chatty: Chatty {
            probability: 50,
            ..chatty_support::chatty()
        },
        supersede_window: 7_000,
        judge: Judge {
            model: None,
            records: 20,
            max_tokens: 400,
            timeout: 60_000,
            moderation_timeout: 120_000,
            retries: 1,
            reason_chars: REASON_CHARS,
        },
        outbound: outbound_support::ctx().outbound,
        split_chars: 3000,
    }
}

#[test]
fn the_repository_defaults_read_without_problems_and_hold_the_old_values() {
    assert_eq!(Params::read(&file(DEFAULTS)), Ok(factory()));
}

#[test]
fn the_factory_chatty_scores_the_two_examples_like_the_hand_written_one() {
    // 18 第七节的两个例子：刚说过话时有人接一句，0.785 对 0.92 不回；同一句 @ 了她，1.085 对 0.8 回。
    let read = defaults();
    let written = Chatty {
        probability: 50,
        ..chatty_support::chatty()
    };
    let judged = Judgement {
        scores: [6.0, 5.0, 4.0, 6.0, 3.0],
        should_reply: true,
        to_bot: false,
        severity: None,
        reason: String::new(),
    };
    let replies = vec![
        chatty_support::reply(0, &[OTHER]),
        chatty_support::reply(0, &[OTHER]),
    ];
    let mut addressed = facts();
    addressed.said.addressed = true;
    for (facts, total, threshold, reply) in [(facts(), 785, 92, false), (addressed, 1085, 80, true)]
    {
        let scored = |chatty: &Chatty| {
            let conditions = hits(&facts, &[], &replies, chatty);
            score(
                &judged,
                &conditions,
                &replies,
                chatty_support::now(),
                chatty,
            )
        };
        let got = scored(&read.chatty);
        assert_eq!(got, scored(&written));
        assert_eq!(mills(got.total), total);
        assert_eq!(cents(got.threshold), threshold);
        assert_eq!(got.reply, reply);
    }
}

#[test]
fn durations_are_handed_out_in_milliseconds() {
    let read = |text: &str| Params::read(&file(text)).expect("读得出");
    let params = read(&edited(
        "continuation_window = \"15s\"",
        "continuation_window = \"2m\"",
    ));
    assert_eq!(params.chatty.continuation.window, 120_000);
    let params = read(&edited("timeout = \"60s\"", "timeout = \"1h\""));
    assert_eq!(params.judge.timeout, 3_600_000);
    // 不写单位是秒。
    let params = read(&edited(
        "supersede_window = \"7s\"",
        "supersede_window = \"9\"",
    ));
    assert_eq!(params.supersede_window, 9_000);
}

#[test]
fn a_missing_item_is_reported_as_wrong_type_without_position() {
    let text = edited("probability = 50", "");
    assert_eq!(problems(&text), [missing("chatty.probability")]);
    // 一整张表没写：每一项都缺。
    let text = edited("[dispatch]\nsupersede_window = \"7s\"", "");
    assert_eq!(problems(&text), [missing("dispatch.supersede_window")]);
}

#[test]
fn the_judge_model_alone_may_be_left_out_and_is_read_when_written() {
    assert_eq!(
        Params::read(&file(DEFAULTS)).map(|p| p.judge.model),
        Ok(None)
    );
    for model in ["@cheap", "deepseek/deepseek-v4-flash"] {
        let text = edited("[judge]\n", &format!("[judge]\nmodel = \"{model}\"\n"));
        let read = Params::read(&file(&text)).map(|p| p.judge.model);
        assert_eq!(read, Ok(Some(model.to_string())));
    }
    let text = edited("[judge]\n", "[judge]\nmodel = \"cheap\"\n");
    let line = line_of(&text, "model");
    assert_eq!(
        problems(&text),
        [bad(Code::BadFormat, "judge.model", line, 9, "\"cheap\"")]
    );
}

#[test]
fn an_unknown_item_or_table_is_reported_with_the_nearest_name() {
    // 多一项：没有离得近的名字。
    let text = edited("base = 0.8", "base = 0.8\nextra_item = 1");
    let line = line_of(&text, "extra_item");
    let unknown = (
        Code::UnknownKey,
        Some("chatty.extra_item".into()),
        Some(At { line, column: 1 }),
        Some("1".into()),
        None,
    );
    assert_eq!(problems(&text), [unknown]);
    // 项名拼错：给这张表里最近的项，写成 `表.项`；拼错的那一项照样缺。
    let text = edited("probability = 50", "probabilty = 50");
    let line = line_of(&text, "probabilty");
    let misspelt = (
        Code::UnknownKey,
        Some("chatty.probabilty".into()),
        Some(At { line, column: 1 }),
        Some("50".into()),
        Some("chatty.probability".into()),
    );
    assert_eq!(problems(&text), [misspelt, missing("chatty.probability")]);
    // 多一张表、表名拼错：给最近的表名。
    let text = format!("{DEFAULTS}\n[chaty]\nbase = 1\n");
    let line = line_of(&text, "[chaty]");
    let table = (
        Code::UnknownKey,
        Some("chaty".into()),
        Some(At { line, column: 2 }),
        Some("chaty".into()),
        Some("chatty".into()),
    );
    assert_eq!(problems(&text), [table]);
    // 最上面一个不是表的键：离哪张表都远。
    let text = format!("probability = 5\n{DEFAULTS}");
    let top = (
        Code::UnknownKey,
        Some("probability".into()),
        Some(At { line: 1, column: 1 }),
        Some("5".into()),
        None,
    );
    assert_eq!(problems(&text), [top]);
}

#[test]
fn a_table_written_as_something_else_is_one_problem_not_one_per_item() {
    // 写在最上面：写在别的表头下面就成了那张表里的一项。
    let text = format!(
        "dispatch = 7\n{}",
        edited("[dispatch]\nsupersede_window = \"7s\"", "")
    );
    assert_eq!(
        problems(&text),
        [bad(Code::WrongType, "dispatch", 1, 12, "7")]
    );
    // 写成表的数组也不是表。
    let text = edited("[dispatch]", "[[dispatch]]");
    let line = line_of(&text, "[[dispatch]]");
    assert_eq!(
        problems(&text),
        [bad(Code::WrongType, "dispatch", line, 3, "dispatch")]
    );
}

#[test]
fn a_badly_written_value_is_reported_once_and_not_again_as_missing() {
    let text = edited("probability = 50", "probability = \"50\"");
    let line = line_of(&text, "probability");
    assert_eq!(
        problems(&text),
        [bad(
            Code::WrongType,
            "chatty.probability",
            line,
            15,
            "\"50\""
        )]
    );
    // 时长写成了整数。
    let text = edited("supersede_window = \"7s\"", "supersede_window = 7");
    assert_eq!(code_of(&text, "dispatch.supersede_window"), Code::WrongType);
    // 开关写成了字。
    let text = edited("restraint = true", "restraint = \"yes\"");
    assert_eq!(code_of(&text, "chatty.restraint"), Code::WrongType);
}

#[test]
fn values_out_of_range_are_reported() {
    for (from, to, key) in [
        (
            "probability = 50",
            "probability = 1001",
            "chatty.probability",
        ),
        ("probability = 50", "probability = -1", "chatty.probability"),
        (
            "severity_min = 7",
            "severity_min = 0",
            "chatty.severity_min",
        ),
        (
            "severity_min = 7",
            "severity_min = 11",
            "chatty.severity_min",
        ),
        ("similar = 66", "similar = 0", "outbound.similar"),
        (
            "min_bigrams = 16",
            "min_bigrams = 0",
            "outbound.min_bigrams",
        ),
        (
            "base64_max_chars = 5000",
            "base64_max_chars = 0",
            "inbound.base64_max_chars",
        ),
        ("max_tokens = 400", "max_tokens = 0", "judge.max_tokens"),
        ("retries = 1", "retries = 11", "judge.retries"),
        ("base = 0.8", "base = 10.5", "chatty.base"),
        ("base = 0.8", "base = -0.1", "chatty.base"),
        ("base = 0.8", "base = nan", "chatty.base"),
        ("base = 0.8", "base = inf", "chatty.base"),
        ("base = 0.8", "base = -inf", "chatty.base"),
        ("relevance = 0.25", "relevance = nan", "chatty.relevance"),
        ("restraint_k = 2.5", "restraint_k = 0", "chatty.restraint_k"),
        (
            "restraint_k = 2.5",
            "restraint_k = 0.0",
            "chatty.restraint_k",
        ),
        (
            "restraint_k = 2.5",
            "restraint_k = -0.0",
            "chatty.restraint_k",
        ),
        (
            "restraint_k = 2.5",
            "restraint_k = 11",
            "chatty.restraint_k",
        ),
        (
            "restraint_cap = 0.35",
            "restraint_cap = inf",
            "chatty.restraint_cap",
        ),
        (
            "mention_after = \"15s\"",
            "mention_after = \"25h\"",
            "outbound.mention_after",
        ),
        ("timeout = \"60s\"", "timeout = \"2h\"", "judge.timeout"),
    ] {
        assert_eq!(code_of(&edited(from, to), key), Code::OutOfRange, "{to}");
    }
}

#[test]
fn the_ends_of_the_ranges_are_accepted() {
    for (from, to) in [
        ("probability = 50", "probability = 0"),
        ("probability = 50", "probability = 1000"),
        ("severity_min = 7", "severity_min = 1"),
        ("severity_min = 7", "severity_min = 10"),
        ("restraint_k = 2.5", "restraint_k = 0.001"),
        ("restraint_k = 2.5", "restraint_k = 10"),
        ("base = 0.8", "base = 0"),
        ("base = 0.8", "base = 10"),
        ("quote_after = 4", "quote_after = 0"),
        ("split_chars = 3000", "split_chars = 0"),
        ("records = 20", "records = 0"),
        ("retries = 1", "retries = 0"),
        ("reason_chars = 500", "reason_chars = 0"),
        ("base64_printable = 850", "base64_printable = 0"),
        ("similar = 66", "similar = 100"),
        ("mention_after = \"15s\"", "mention_after = \"1s\""),
        ("mention_after = \"15s\"", "mention_after = \"24h\""),
        ("timeout = \"60s\"", "timeout = \"1h\""),
    ] {
        assert!(Params::read(&file(&edited(from, to))).is_ok(), "{to}");
    }
}

#[test]
fn a_zero_duration_is_not_a_duration() {
    for (from, to, key) in [
        (
            "continuation_window = \"15s\"",
            "continuation_window = \"0s\"",
            "chatty.continuation_window",
        ),
        (
            "restraint_half_life = \"3m\"",
            "restraint_half_life = \"0m\"",
            "chatty.restraint_half_life",
        ),
        (
            "supersede_window = \"7s\"",
            "supersede_window = \"0\"",
            "dispatch.supersede_window",
        ),
        ("timeout = \"60s\"", "timeout = \"1.5s\"", "judge.timeout"),
    ] {
        assert_eq!(code_of(&edited(from, to), key), Code::BadFormat, "{to}");
    }
}

#[test]
fn a_syntax_error_is_the_only_problem() {
    let seen = problems(&format!("{DEFAULTS}\n[chatty\n"));
    assert_eq!(seen.len(), 1, "{seen:?}");
    assert_eq!(seen[0].0, Code::Syntax);
}

#[test]
fn problems_follow_lines_and_missing_items_come_last_in_declared_order() {
    let text = edited("similar = 66", "similar = 0")
        .replace("probability = 50", "probability = 2000")
        .replace("records = 20", "")
        .replace("base64_min_chars = 24", "");
    let seen: Vec<_> = problems(&text)
        .into_iter()
        .map(|(code, key, ..)| (code, key.unwrap_or_default()))
        .collect();
    assert_eq!(
        seen,
        [
            (Code::OutOfRange, "chatty.probability".to_string()),
            (Code::OutOfRange, "outbound.similar".to_string()),
            (Code::WrongType, "inbound.base64_min_chars".to_string()),
            (Code::WrongType, "judge.records".to_string()),
        ]
    );
}

#[test]
fn problems_are_sorted_even_when_the_table_order_differs_from_the_lines() {
    // 点号连着写的 `inbound.…` 在表里排在一起，问题照行排：第 1、2、3 行。缺了的跟在后面。
    let text = "inbound.base64_min_chars = 0\noutbound.similar = 0\ninbound.base64_max_chars = 0\n";
    let seen: Vec<_> = problems(text)
        .into_iter()
        .take(4)
        .map(|(code, key, at, ..)| (code, key.unwrap_or_default(), at.map(|at| at.line)))
        .collect();
    let one = |code, key: &str, line| (code, key.to_string(), line);
    assert_eq!(
        seen,
        [
            one(Code::OutOfRange, "inbound.base64_min_chars", Some(1)),
            one(Code::OutOfRange, "outbound.similar", Some(2)),
            one(Code::OutOfRange, "inbound.base64_max_chars", Some(3)),
            one(Code::WrongType, "inbound.base64_printable", None),
        ]
    );
}

#[test]
fn a_bom_is_stripped_and_positions_skip_it() {
    let text = format!("\u{FEFF}{}", edited("similar = 66", "similar = 0"));
    let line = line_of(&text, "similar");
    assert_eq!(
        problems(&text),
        [bad(Code::OutOfRange, "outbound.similar", line, 11, "0")]
    );
    assert!(Params::read(&file(&format!("\u{FEFF}{DEFAULTS}"))).is_ok());
}

#[test]
fn a_warning_alone_still_rejects_the_whole_file() {
    // 多一项在配置里只是警告；出厂文件是打包的，一样整份不用。
    let text = edited("similar = 66", "similar = 66\nsimilarity = 66");
    let seen = problems(&text);
    assert_eq!(seen.len(), 1, "{seen:?}");
    assert_eq!(seen[0].0, Code::UnknownKey);
}
