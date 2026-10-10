//! 记忆的测评集（施工 R-11，`docs/blueprint/memory/eval.md`）：读 `tests/fixtures/memory-eval/` 的两份、照规矩查、照搜出来的
//! 算分。数据和规矩见那一页「格式」；这里只管读、查、算，不碰记忆。

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::Deserialize;

use miyu_kernel::time::Timestamp;

/// 六种问句，照蓝图的先后：表也照它排。
pub const KINDS: [&str; 6] = ["keyword", "paraphrase", "chat", "cross", "changed", "none"];

/// 一条记忆。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Memory {
    /// 问句指它用的键。
    pub key: String,
    /// 四类之一。
    pub class: String,
    /// 记下的那一天，`YYYY-MM-DD`。
    pub at: String,
    /// 正文。
    pub text: String,
    /// 作废的为什么；没作废的没有。
    #[serde(default)]
    pub retired: Option<String>,
}

/// 一句问句。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    /// 人说的话，或者她搜的词。
    pub text: String,
    /// 六种之一（[`KINDS`]）。
    pub kind: String,
    /// 该想起的那几条的键；`none` 的是空的。
    pub expect: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Memories {
    memory: Vec<Memory>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Queries {
    query: Vec<Query>,
}

/// 一份测评集。
#[derive(Debug, Clone)]
pub struct EvalSet {
    pub memories: Vec<Memory>,
    pub queries: Vec<Query>,
}

/// 仓库里的那一份。
pub fn shipped() -> EvalSet {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/memory-eval");
    let read = |name: &str| std::fs::read_to_string(dir.join(name)).expect("读得到");
    parse(&read("memories.toml"), &read("queries.toml")).unwrap_or_else(|why| panic!("{why}"))
}

/// 照两份的字读：读不进的、多写了格的交回为什么。
pub fn parse(memories: &str, queries: &str) -> Result<EvalSet, String> {
    let memories: Memories =
        toml_edit::de::from_str(memories).map_err(|error| format!("memories.toml：{error}"))?;
    let queries: Queries =
        toml_edit::de::from_str(queries).map_err(|error| format!("queries.toml：{error}"))?;
    Ok(EvalSet {
        memories: memories.memory,
        queries: queries.query,
    })
}

/// 那一天上午九点（UTC）：记下的时刻。写错的没有。
pub fn day(at: &str) -> Option<Timestamp> {
    let shaped = at.len() == 10
        && at.char_indices().all(|(n, c)| {
            if n == 4 || n == 7 {
                c == '-'
            } else {
                c.is_ascii_digit()
            }
        });
    shaped
        .then(|| Timestamp::parse(&format!("{at}T09:00:00.000Z")).ok())
        .flatten()
}

/// 连着的两个字：小写，只算字母、数字、汉字假名这些（`char::is_alphanumeric`），隔着空白、标点的不连。`paraphrase`、`cross`
/// 照它判「一个字都不重合」，`keyword` 照它判有重合：关键词那一路是两字切分（`recall.md` 第一条）。
pub fn pairs(text: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let lower = text.to_lowercase();
    for run in lower.split(|c: char| !c.is_alphanumeric()) {
        let chars: Vec<char> = run.chars().collect();
        for pair in chars.windows(2) {
            found.insert(pair.iter().collect());
        }
    }
    found
}

/// 照蓝图「格式」的规矩逐条查，交回不合的每一处（一处一句）。空的就是合规矩。
pub fn problems(set: &EvalSet) -> Vec<String> {
    let mut problems = Vec::new();
    let mut by_key: BTreeMap<&str, &Memory> = BTreeMap::new();
    for memory in &set.memories {
        let key = memory.key.as_str();
        if key.is_empty()
            || !key
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            problems.push(format!("{key}：键只用小写字母、数字、-"));
        }
        if by_key.insert(key, memory).is_some() {
            problems.push(format!("{key}：键重了"));
        }
        if !miyu_recall::CLASSES.contains(&memory.class.as_str()) {
            problems.push(format!("{key}：不认识的类 {}", memory.class));
        }
        if day(&memory.at).is_none() {
            problems.push(format!("{key}：日期写错了 {}", memory.at));
        }
        let chars = memory.text.trim().chars().count();
        if chars == 0 || chars > miyu_tool::TEXT_CHARS {
            problems.push(format!("{key}：正文 {chars} 字"));
        }
        if memory
            .retired
            .as_deref()
            .is_some_and(|why| why.trim().is_empty())
        {
            problems.push(format!("{key}：作废的要写为什么"));
        }
    }
    for query in &set.queries {
        problems.extend(query_problems(query, &by_key));
    }
    let (memories, queries) = (set.memories.len(), set.queries.len());
    if !(90..=120).contains(&memories) {
        problems.push(format!("记忆 {memories} 条，要 90 到 120"));
    }
    if !(45..=60).contains(&queries) {
        problems.push(format!("问句 {queries} 句，要 45 到 60"));
    }
    let none = set
        .queries
        .iter()
        .filter(|query| query.kind == "none")
        .count();
    if none * 5 < queries {
        problems.push(format!("none 只有 {none} 句，要占两成以上"));
    }
    for kind in KINDS {
        if !set.queries.iter().any(|query| query.kind == kind) {
            problems.push(format!("没有 {kind} 的问句"));
        }
    }
    problems
}

/// 一句问句不合的几处。
fn query_problems(query: &Query, by_key: &BTreeMap<&str, &Memory>) -> Vec<String> {
    let mut problems = Vec::new();
    let text = &query.text;
    if text.trim().is_empty() {
        problems.push("有一句问句是空的".to_string());
    }
    if !KINDS.contains(&query.kind.as_str()) {
        problems.push(format!("{text}：不认识的种类 {}", query.kind));
    }
    if (query.kind == "none") != query.expect.is_empty() {
        problems.push(format!("{text}：none 的才不写该想起的"));
    }
    let mut seen = BTreeSet::new();
    let mut expected = Vec::new();
    for key in &query.expect {
        if !seen.insert(key) {
            problems.push(format!("{text}：{key} 写了两次"));
        }
        match by_key.get(key.as_str()) {
            None => problems.push(format!("{text}：没有 {key} 这一条")),
            Some(memory) if memory.retired.is_some() => {
                problems.push(format!("{text}：{key} 作废了，不该想起"));
            }
            Some(memory) => expected.push(*memory),
        }
    }
    let asked = pairs(text);
    let shares = |memory: &Memory| !asked.is_disjoint(&pairs(&memory.text));
    match query.kind.as_str() {
        "paraphrase" | "cross" => {
            for memory in expected.iter().filter(|memory| shares(memory)) {
                problems.push(format!("{text}：和 {} 有两个字连着重合", memory.key));
            }
        }
        "keyword" if !expected.iter().any(|memory| shares(memory)) => {
            problems.push(format!("{text}：和该想起的没有两个字连着重合"));
        }
        _ => {}
    }
    problems
}

/// 一句搜出来的：种类、该想起的、搜出来的前几条（照名次）。
#[derive(Debug, Clone)]
pub struct Found {
    pub kind: String,
    pub expect: Vec<String>,
    pub got: Vec<String>,
}

/// 表里的一行：一种问句，或者有答案的几种合在一起（`all`）。
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub kind: String,
    /// 几句。
    pub queries: usize,
    /// 头一条就是该想起的，占几成（`none` 的不算）。
    pub hit1: f64,
    /// 前 3 条里找到了该想起的几成，各句平均。
    pub recall3: f64,
    /// 前 5 条里，同上。
    pub recall5: f64,
    /// 头一个该想起的排第几的倒数，各句平均；前 5 条都没有的算 0。
    pub mrr: f64,
    /// 搜出了东西的占几成（`none` 的看这一格：该是 0）。
    pub any: f64,
    /// 平均搜出几条。
    pub mean: f64,
}

/// 照种类算分，照 [`KINDS`] 的先后，最后一行 `all` 是有答案的几种合在一起；一句都没有的种类不出行。
pub fn score(found: &[Found]) -> Vec<Row> {
    let mut rows: Vec<Row> = KINDS
        .iter()
        .filter_map(|kind| row(kind, found.iter().filter(|one| one.kind == *kind)))
        .collect();
    rows.extend(row(
        "all",
        found.iter().filter(|one| !one.expect.is_empty()),
    ));
    rows
}

fn row<'a>(kind: &str, found: impl Iterator<Item = &'a Found>) -> Option<Row> {
    let found: Vec<&Found> = found.collect();
    if found.is_empty() {
        return None;
    }
    let count = found.len() as f64;
    let average = |of: &dyn Fn(&Found) -> f64| found.iter().map(|one| of(one)).sum::<f64>() / count;
    let recall = |one: &Found, k: usize| {
        if one.expect.is_empty() {
            return 0.0;
        }
        let top: BTreeSet<&String> = one.got.iter().take(k).collect();
        one.expect.iter().filter(|key| top.contains(key)).count() as f64 / one.expect.len() as f64
    };
    Some(Row {
        kind: kind.to_string(),
        queries: found.len(),
        hit1: average(&|one| {
            f64::from(u8::from(
                one.got.first().is_some_and(|top| one.expect.contains(top)),
            ))
        }),
        recall3: average(&|one| recall(one, 3)),
        recall5: average(&|one| recall(one, 5)),
        mrr: average(&|one| {
            one.got
                .iter()
                .take(5)
                .position(|key| one.expect.contains(key))
                .map_or(0.0, |rank| 1.0 / (rank + 1) as f64)
        }),
        any: average(&|one| f64::from(u8::from(!one.got.is_empty()))),
        mean: average(&|one| one.got.len() as f64),
    })
}

/// 印成一张表（蓝图里抄的就是它）：百分数取整，MRR、平均条数两位小数。
pub fn table(title: &str, rows: &[Row]) -> String {
    let mut out = format!(
        "{title}\n| 种类 | 句数 | hit@1 | recall@3 | recall@5 | MRR | 搜出东西 | 平均条数 |\n|---|---|---|---|---|---|---|---|\n"
    );
    let percent = |share: f64| format!("{:.0}%", share * 100.0);
    for row in rows {
        let answered = row.kind != "none";
        let cell = |share: f64| {
            if answered {
                percent(share)
            } else {
                "—".to_string()
            }
        };
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {:.2} |\n",
            row.kind,
            row.queries,
            cell(row.hit1),
            cell(row.recall3),
            cell(row.recall5),
            if answered {
                format!("{:.2}", row.mrr)
            } else {
                "—".to_string()
            },
            percent(row.any),
            row.mean,
        ));
    }
    out
}

/// 一句联想挑中的（施工 R-8）：种类、该想起的、挑中的（照先后）。
#[derive(Debug, Clone)]
pub struct Picked {
    pub kind: String,
    pub expect: Vec<String>,
    pub picked: Vec<String>,
}

/// 联想那张表的一行：一种问句，或者有答案的几种合在一起（`all`）。
#[derive(Debug, Clone, PartialEq)]
pub struct Brought {
    pub kind: String,
    /// 几句。
    pub queries: usize,
    /// 带了东西的占几成（`none` 的看这一格：该是 0）。
    pub fired: f64,
    /// 带了的里正是该想起的占几成；一条都没带的没有。
    pub precision: Option<f64>,
    /// 至少带对一条的占几成（`none` 的不算）。
    pub hit: f64,
    /// 平均带几条。
    pub mean: f64,
}

/// 照种类算联想的分，照 [`KINDS`] 的先后，最后一行 `all` 是有答案的几种合在一起；一句都没有的种类不出行。
pub fn brought(picked: &[Picked]) -> Vec<Brought> {
    let mut rows: Vec<Brought> = KINDS
        .iter()
        .filter_map(|kind| brought_row(kind, picked.iter().filter(|one| one.kind == *kind)))
        .collect();
    rows.extend(brought_row(
        "all",
        picked.iter().filter(|one| !one.expect.is_empty()),
    ));
    rows
}

fn brought_row<'a>(kind: &str, picked: impl Iterator<Item = &'a Picked>) -> Option<Brought> {
    let picked: Vec<&Picked> = picked.collect();
    if picked.is_empty() {
        return None;
    }
    let count = picked.len() as f64;
    let total: usize = picked.iter().map(|one| one.picked.len()).sum();
    let right = |one: &Picked| {
        one.picked
            .iter()
            .filter(|key| one.expect.contains(key))
            .count()
    };
    let correct: usize = picked.iter().map(|one| right(one)).sum();
    Some(Brought {
        kind: kind.to_string(),
        queries: picked.len(),
        fired: picked.iter().filter(|one| !one.picked.is_empty()).count() as f64 / count,
        precision: (total > 0).then(|| correct as f64 / total as f64),
        hit: picked.iter().filter(|one| right(one) > 0).count() as f64 / count,
        mean: total as f64 / count,
    })
}

/// 印成一张表：百分数取整，平均条数两位小数；`none` 的「带对」写「—」，没带的「准」写「—」。
pub fn brought_table(title: &str, rows: &[Brought]) -> String {
    let mut out = format!(
        "{title}\n| 种类 | 句数 | 带了东西 | 带了的里该带的 | 这一轮带对了 | 平均带几条 |\n|---|---|---|---|---|---|\n"
    );
    let percent = |share: f64| format!("{:.0}%", share * 100.0);
    for row in rows {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {:.2} |\n",
            row.kind,
            row.queries,
            percent(row.fired),
            row.precision.map_or("—".to_string(), percent),
            if row.kind == "none" {
                "—".to_string()
            } else {
                percent(row.hit)
            },
            row.mean,
        ));
    }
    out
}
