//! 池（`models.md`「守着它的」`pools/tests.rs`，施工 8-8）：钉住的从日志读回、轮换的指针、成员变了取余、不写分法时怎么分；
//! 认不出的成员跳过、一个都不剩的解析不出；候选绕回到头。

use super::*;
use crate::test_support::resolved;

fn values(source: &str) -> Values {
    resolved(source).values()
}

/// `a`、`b` 两家，`b` 写了按次计费；池 `p` 的成员照 `members` 写，分法照 `strategy`（空的不写）。
fn config(members: &str, strategy: &str) -> Values {
    let strategy = match strategy {
        "" => String::new(),
        written => format!("strategy = \"{written}\"\n"),
    };
    values(&format!(
        "[providers.a]\nkeys = []\n\n[providers.b]\nkeys = []\ncache = \"per_request\"\n\n[pools.p]\nmodels = [{members}]\n{strategy}"
    ))
}

#[test]
fn unwritten_strategy_rotates_only_when_every_member_is_per_request() {
    let pool =
        |members: &str, strategy: &str| pool(&config(members, strategy), "p").expect("解析得出");
    assert_eq!(
        pool(r#""b/x", "b/y""#, "").strategy,
        Strategy::Rotate,
        "全是按次计费的"
    );
    assert_eq!(
        pool(r#""b/x", "a/y""#, "").strategy,
        Strategy::Pin,
        "有一家不是"
    );
    assert_eq!(
        pool(r#""a/x""#, "").strategy,
        Strategy::Pin,
        "没写 cache 的不算"
    );
    assert_eq!(
        pool(r#""a/x""#, "rotate").strategy,
        Strategy::Rotate,
        "写了的照写的"
    );
    assert_eq!(pool(r#""b/x""#, "pin").strategy, Strategy::Pin);
    assert_eq!(Strategy::Pin.as_str(), "pin");
    assert_eq!(Strategy::Rotate.as_str(), "rotate");
}

#[test]
fn unknown_members_are_skipped_and_an_empty_pool_does_not_resolve() {
    let found = pool(&config(r#""gone/x", "a/y", "c/z""#, ""), "p").expect("还剩一个");
    assert_eq!(
        found.members,
        [Member {
            provider: "a".to_string(),
            model: "y".to_string()
        }]
    );
    assert_eq!(found.skipped, ["gone/x", "c/z"]);
    assert_eq!(
        pool(&config(r#""gone/x""#, ""), "p"),
        Err(NoModel(r#"pool "p" has no models"#.to_string()))
    );
    let unwritten = values("[providers.a]\nkeys = []\n\n[pools.p]\nstrategy = \"pin\"\n");
    assert_eq!(
        pool(&unwritten, "p"),
        Err(NoModel(r#"pool "p" has no models"#.to_string())),
        "成员没写"
    );
    assert_eq!(
        pool(&config(r#""a/y""#, ""), "q"),
        Err(NoModel(r#"no pool "q""#.to_string()))
    );
    assert_eq!(names(&config(r#""a/y""#, "")), ["p"]);
}

/// 钉住的从日志读回：最近一条发出去了的是这个池的成员，钉着它；不是的没有。
#[test]
fn a_pinned_member_is_found_again_from_what_was_sent() {
    let found = pool(&config(r#""a/x", "b/y", "a/y""#, ""), "p").expect("解析得出");
    assert_eq!(found.find("b", "y"), Some(1));
    assert_eq!(found.find("a", "y"), Some(2));
    assert_eq!(found.find("b", "x"), None, "家和模型都要对上");
    assert_eq!(found.order(1), [1, 2, 0], "从它起，绕回到头");
    assert_eq!(found.order(4), [1, 2, 0], "超了的取余");
}

#[test]
fn the_pointer_walks_round_and_follows_a_changed_member_count() {
    let mut pointers = Pointers::default();
    let taken: Vec<usize> = (0..5).map(|_| pointers.take("p", 3)).collect();
    assert_eq!(taken, [0, 1, 2, 0, 1]);
    assert_eq!(pointers.take("q", 2), 0, "一个池一个指针");
    assert_eq!(pointers.to_json(), r#"{"p":2,"q":1}"#);
    // 成员少了：指针对新的个数取余。
    assert_eq!(pointers.take("p", 2), 0);
    assert_eq!(pointers.take("p", 2), 1);
    let read = Pointers::parse(r#"{"p":7}"#).expect("读得进");
    let mut read = read;
    assert_eq!(read.take("p", 3), 1, "7 对 3 取余");
    assert_eq!(read.take("p", 0), 0, "没有成员的不动");
    assert_eq!(read.to_json(), r#"{"p":2}"#);
    assert!(Pointers::parse("[1]").is_err());
    assert!(Pointers::parse(r#"{"p":-1}"#).is_err());
}

/// 派子代理能选的池（施工 8-8 补）：开关开着、至少有一个认得出的成员的才列，照名字的字节序排，说明照写的。
#[test]
fn subagents_are_offered_switched_on_pools_with_members_by_name() {
    let config = values(
        "[providers.a]\nkeys = []\n\n\
         [pools.zeta]\nmodels = [\"a/z\"]\nsubagent = true\n\n\
         [pools.alpha]\nmodels = [\"a/x\"]\nsubagent = true\ndescription = \"Quick lookups.\"\n\n\
         [pools.off]\nmodels = [\"a/y\"]\n\n\
         [pools.lite]\nmodels = []\nsubagent = true\n\n\
         [pools.gone]\nmodels = [\"nope/y\"]\nsubagent = true\n",
    );
    let listed = offered(&config);
    let names: Vec<&str> = listed.iter().map(|offer| offer.name.as_str()).collect();
    assert_eq!(
        names,
        ["alpha", "zeta"],
        "没开的、空的、一个成员都认不出的不列"
    );
    assert_eq!(listed[0].description.as_deref(), Some("Quick lookups."));
    assert_eq!(listed[1].description, None);
    assert!(offered(&values("")).is_empty(), "一个池都没有");
}

#[test]
fn offered_names_sort_by_bytes() {
    let config = values(
        "[providers.a]\nkeys = []\n\n[pools.b]\nmodels = [\"a/x\"]\nsubagent = true\n\n\
         [pools.a-1]\nmodels = [\"a/x\"]\nsubagent = true\n\n[pools.a_1]\nmodels = [\"a/x\"]\nsubagent = true\n",
    );
    let names: Vec<String> = offered(&config)
        .into_iter()
        .map(|offer| offer.name)
        .collect();
    assert_eq!(names, ["a-1", "a_1", "b"]);
}
