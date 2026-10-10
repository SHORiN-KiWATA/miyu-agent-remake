//! 记忆的测评集（施工 R-11，`docs/blueprint/memory/eval.md`）：仓库里那一份合规矩，故意写坏的几种查得出来，分照定义算；量尺
//! （`#[ignore]`）照每条记下的日期记进一间、作废的作废，每句照 `Keeper::search` 搜前 5 条，先只走关键词，设了
//! `MIYU_EMBED_MODEL_DIR` 的再接上真的 bge 两路合并搜一遍，照种类印表。
//!
//! 跑量尺：`MIYU_EMBED_MODEL_DIR=<Release 的文件> cargo test -p miyu-session --release --test all memory_eval -- --ignored
//! --nocapture`（不设只量关键词那一路）。

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use miyu_session::{EmbedSetup, Embedder, Keeper, Stamp, Turn, Using, Vectors};
use miyu_tool::Remember;

use crate::support::eval::{EvalSet, Found, Row, day, parse, problems, score, shipped, table};
use crate::support::meaning::{filled, model_data, persona};
use crate::support::package::program;
use crate::support::{Home, alice, alice_account};

#[test]
fn the_eval_set_is_well_formed() {
    let problems = problems(&shipped());
    assert!(problems.is_empty(), "{problems:#?}");
}

/// 照仓库里那一份改一处，交回查出来的。
fn broken(change: impl FnOnce(&mut EvalSet)) -> Vec<String> {
    let mut set = shipped();
    change(&mut set);
    problems(&set)
}

fn caught(problems: &[String], what: &str) {
    assert!(
        problems.iter().any(|problem| problem.contains(what)),
        "该查出「{what}」：{problems:#?}"
    );
}

/// `kind` 这一种的头一句在第几。
fn query_of(set: &EvalSet, kind: &str) -> usize {
    set.queries
        .iter()
        .position(|query| query.kind == kind)
        .expect("有这一种")
}

#[test]
fn broken_sets_are_caught() {
    caught(
        &broken(|set| set.memories[1].key = set.memories[0].key.clone()),
        "键重了",
    );
    caught(
        &broken(|set| set.memories[0].key = "Cat Name".into()),
        "键只用小写字母",
    );
    caught(
        &broken(|set| set.memories[0].class = "secret".into()),
        "不认识的类",
    );
    caught(
        &broken(|set| set.memories[0].at = "2026/03/02".into()),
        "日期写错了",
    );
    caught(
        &broken(|set| set.memories[0].text = "长".repeat(121)),
        "正文 121 字",
    );
    caught(
        &broken(|set| set.memories[0].retired = Some(" ".into())),
        "作废的要写为什么",
    );
    caught(
        &broken(|set| {
            let n = query_of(set, "changed");
            set.queries[n].expect = vec!["gpu-old".into()];
        }),
        "作废了，不该想起",
    );
    caught(
        &broken(|set| {
            let n = query_of(set, "none");
            set.queries[n].expect = vec!["cat-name".into()];
        }),
        "none 的才不写该想起的",
    );
    caught(
        &broken(|set| {
            let n = query_of(set, "chat");
            set.queries[n].expect = Vec::new();
        }),
        "none 的才不写该想起的",
    );
    caught(
        &broken(|set| {
            let n = query_of(set, "chat");
            set.queries[n].expect = vec!["cat-food".into(), "cat-food".into()];
        }),
        "写了两次",
    );
    caught(
        &broken(|set| {
            let n = query_of(set, "chat");
            set.queries[n].expect = vec!["no-such".into()];
        }),
        "没有 no-such 这一条",
    );
    caught(
        &broken(|set| {
            let n = query_of(set, "chat");
            set.queries[n].kind = "smalltalk".into();
        }),
        "不认识的种类",
    );
    caught(
        &broken(|set| {
            let n = query_of(set, "paraphrase");
            set.queries[n].text = "团子是谁".into();
            set.queries[n].expect = vec!["cat-name".into()];
        }),
        "两个字连着重合",
    );
    caught(
        &broken(|set| {
            let n = query_of(set, "keyword");
            set.queries[n].text = "完全不相干的话".into();
        }),
        "没有两个字连着重合",
    );
    caught(&broken(|set| set.memories.truncate(80)), "记忆 80 条");
    caught(&broken(|set| set.queries.truncate(40)), "问句 40 句");
    caught(
        &broken(|set| set.queries.retain(|query| query.kind != "cross")),
        "没有 cross 的问句",
    );
    caught(
        &broken(|set| {
            let keep: Vec<_> = set.queries.drain(..).collect();
            let mut none = 0;
            set.queries = keep
                .into_iter()
                .filter(|query| {
                    query.kind != "none" || {
                        none += 1;
                        none <= 5
                    }
                })
                .collect();
        }),
        "要占两成以上",
    );
    let extra = parse(
        "[[memory]]\nkey = \"a\"\nclass = \"user\"\nat = \"2026-01-01\"\ntext = \"x\"\nsource = \"s\"\n",
        "",
    );
    assert!(extra.is_err(), "多写了格的读不进");
}

#[test]
fn scores_follow_their_definitions() {
    let found = |kind: &str, expect: &[&str], got: &[&str]| Found {
        kind: kind.into(),
        expect: expect.iter().map(|key| key.to_string()).collect(),
        got: got.iter().map(|key| key.to_string()).collect(),
    };
    let rows = score(&[
        found("keyword", &["a"], &["a", "b"]),
        found("keyword", &["a", "b"], &["c", "b", "d", "a", "e"]),
        found("none", &[], &[]),
        found("none", &[], &["x", "y"]),
    ]);
    let keyword = Row {
        kind: "keyword".into(),
        queries: 2,
        hit1: 0.5,
        recall3: 0.75,
        recall5: 1.0,
        mrr: 0.75,
        any: 1.0,
        mean: 3.5,
    };
    assert_eq!(
        rows,
        [
            keyword.clone(),
            Row {
                kind: "none".into(),
                queries: 2,
                hit1: 0.0,
                recall3: 0.0,
                recall5: 0.0,
                mrr: 0.0,
                any: 0.5,
                mean: 1.0,
            },
            Row {
                kind: "all".into(),
                ..keyword
            },
        ]
    );
}

/// 每句照 `near` 交的向量搜前 5 条，交回搜出来的键。
async fn run(
    keeper: &Keeper,
    set: &EvalSet,
    keys: &BTreeMap<String, String>,
    vectors: Option<(&Vectors, &Using)>,
) -> Vec<Found> {
    let mut found = Vec::new();
    for query in &set.queries {
        let near = match vectors {
            Some((vectors, using)) => {
                let near = vectors.query(using, &query.text).await;
                assert!(near.is_some(), "算得出问句的向量：{}", query.text);
                near
            }
            None => None,
        };
        let got = keeper
            .search(&query.text, false, 5, near.as_ref())
            .expect("搜得了")
            .iter()
            .map(|entry| keys[&entry.id.to_string()].clone())
            .collect();
        found.push(Found {
            kind: query.kind.clone(),
            expect: query.expect.clone(),
            got,
        });
    }
    found
}

/// 作废的一条都没搜出来，每句都搜了。
fn sound(set: &EvalSet, found: &[Found]) {
    assert_eq!(found.len(), set.queries.len());
    for one in found {
        for key in &one.got {
            let memory = set.memories.iter().find(|memory| &memory.key == key);
            assert!(
                memory.is_some_and(|memory| memory.retired.is_none()),
                "作废的 {key} 搜出来了"
            );
        }
    }
}

/// Release 的文件摆成内置语义模型的包目录：模型清单照出厂的。
fn real_package(dir: &Path, files: &Path) -> EmbedSetup {
    std::fs::create_dir_all(dir).expect("建得了");
    let shipped =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../miyu-embed/package/embed/model.toml");
    std::fs::copy(shipped, dir.join("model.toml")).expect("拷得了");
    for name in ["model_quantized.onnx", "vocab.txt"] {
        std::fs::copy(files.join(name), dir.join(name)).expect("Release 的文件在");
    }
    EmbedSetup {
        program: Some(program()),
        manifest: dir.join("model.toml"),
        dir: dir.to_path_buf(),
        idle: Duration::from_secs(600),
    }
}

#[tokio::test]
#[ignore = "量尺：设了 MIYU_EMBED_MODEL_DIR 的连照意思那一路一起量"]
async fn measure_the_eval_set() {
    let set = shipped();
    let home = Home::new();
    let keeper = Keeper::new(&home.memory, persona(), vec![alice()]);
    let mut keys = BTreeMap::new();
    for memory in &set.memories {
        let stamp = Stamp {
            at: day(&memory.at).expect("合写法"),
            by: alice(),
            cause: None,
        };
        let remember = Remember {
            class: memory.class.clone(),
            text: memory.text.clone(),
            replaces: None,
        };
        let id = keeper
            .save(stamp.clone(), remember, Vec::new())
            .expect("记得下");
        if let Some(why) = &memory.retired {
            keeper.retire(stamp, id, why.clone()).expect("作废得了");
        }
        keys.insert(id.to_string(), memory.key.clone());
    }
    let keyword = run(&keeper, &set, &keys, None).await;
    sound(&set, &keyword);
    println!("{}", table("关键词", &score(&keyword)));
    let Some(files) = std::env::var_os("MIYU_EMBED_MODEL_DIR") else {
        println!("没设 MIYU_EMBED_MODEL_DIR：只量了关键词那一路");
        return;
    };
    let setup = real_package(&home.scratch.0.join("packages/embed"), Path::new(&files));
    let vectors = Arc::new(Vectors::new(Some(Embedder::new(setup)), model_data(&home)));
    assert!(home.memory.give_vectors(Arc::clone(&vectors)), "头一次接");
    let using = Using {
        config: Arc::new(Turn::new(
            Default::default(),
            Arc::clone(&*home.configs.borrow()),
        )),
        owner: alice_account(),
    };
    keeper.fill(&using);
    for (id, key) in &keys {
        if set
            .memories
            .iter()
            .any(|memory| &memory.key == key && memory.retired.is_none())
        {
            filled(&home, "local:bge-small-zh-v1.5", true, id).await;
        }
    }
    let both = run(&keeper, &set, &keys, Some((&vectors, &using))).await;
    sound(&set, &both);
    println!("{}", table("关键词加 bge-small-zh-v1.5", &score(&both)));
}
