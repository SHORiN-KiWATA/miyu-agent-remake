//! 合并交回来的怎么读：前后多的话不管、几条、一条不合的丢掉别的照收、又改又作废的照作废、摘要空的当没写、超长的截在句子
//! 边界上、读不成的整次失败。

use super::*;

fn id(n: u64) -> MemoryId {
    MemoryId::parse(&format!("m{n}")).expect("合写法")
}

fn given() -> BTreeSet<MemoryId> {
    [id(3), id(7), id(12)].into_iter().collect()
}

#[test]
fn the_object_is_found_inside_extra_words() {
    let answer = "Sure:\n```json\n{\"revised\": [{\"id\": \"m3\", \"text\": \" 用户 2026-10-08 去了京都 \"}], \"retired\": [{\"id\": \"m7\", \"why\": \"重复 m12\"}], \"summary\": \" 用户住在东京。\"}\n```";
    assert_eq!(
        decisions(answer, &given(), 120, 600).expect("读得成"),
        Decisions {
            revised: vec![Revised {
                id: id(3),
                text: "用户 2026-10-08 去了京都".into(),
            }],
            retired: vec![Retire {
                id: id(7),
                why: "重复 m12".into(),
            }],
            summary: Some("用户住在东京。".into()),
        }
    );
}

#[test]
fn missing_lists_are_empty_and_an_empty_summary_is_none() {
    assert_eq!(
        decisions(r#"{"summary": "  "}"#, &given(), 120, 600).expect("读得成"),
        Decisions::default()
    );
    assert_eq!(
        decisions(r#"{"revised": [], "retired": []}"#, &given(), 120, 600).expect("读得成"),
        Decisions::default()
    );
}

#[test]
fn a_bad_entry_is_dropped_and_the_rest_kept() {
    let long = "长".repeat(121);
    let answer = format!(
        r#"{{"revised": [
            {{"id": "m3", "text": "好的一条"}},
            {{"id": "m4", "text": "不是交进去的"}},
            {{"id": "m12", "text": "  "}},
            {{"id": "m12", "text": "{long}"}},
            {{"id": "3", "text": "编号写错了"}},
            {{"text": "没有编号"}},
            {{"id": "m3", "text": "同一条第二次"}},
            "不是对象"
        ], "retired": [
            {{"id": "m7", "why": " "}},
            {{"id": "m7"}},
            {{"id": "m12", "why": "被推翻了"}},
            {{"id": "m12", "why": "第二次"}}
        ]}}"#
    );
    let read = decisions(&answer, &given(), 120, 600).expect("读得成");
    assert_eq!(
        read.revised,
        [Revised {
            id: id(3),
            text: "好的一条".into()
        }]
    );
    assert_eq!(
        read.retired,
        [Retire {
            id: id(12),
            why: "被推翻了".into()
        }]
    );
    assert_eq!(
        decisions(
            &format!(
                r#"{{"revised": [{{"id": "m3", "text": "{}"}}]}}"#,
                "长".repeat(120)
            ),
            &given(),
            120,
            600
        )
        .expect("读得成")
        .revised
        .len(),
        1,
        "120 个字收"
    );
}

#[test]
fn revised_and_retired_at_once_is_retired() {
    let answer = r#"{"revised": [{"id": "m7", "text": "改一下"}], "retired": [{"id": "m7", "why": "过时了"}]}"#;
    let read = decisions(answer, &given(), 120, 600).expect("读得成");
    assert_eq!(read.revised, []);
    assert_eq!(read.retired.len(), 1);
}

#[test]
fn a_long_summary_is_cut_at_a_sentence() {
    let answer = format!(
        r#"{{"summary": "{}。第二句很长{}"}}"#,
        "甲".repeat(5),
        "乙".repeat(20)
    );
    assert_eq!(
        decisions(&answer, &given(), 120, 10)
            .expect("读得成")
            .summary,
        Some("甲甲甲甲甲。".into()),
        "截在句子边界上"
    );
    let answer = format!(r#"{{"summary": "{}"}}"#, "丙".repeat(20));
    assert_eq!(
        decisions(&answer, &given(), 120, 10)
            .expect("读得成")
            .summary,
        Some("丙".repeat(10)),
        "一句都放不下的照字截"
    );
    let answer = r#"{"summary": "One. Two three four five six."}"#;
    assert_eq!(
        decisions(answer, &given(), 120, 12)
            .expect("读得成")
            .summary,
        Some("One.".into()),
        "英文的句号也算"
    );
}

#[test]
fn an_unreadable_answer_fails_as_a_whole() {
    for answer in [
        "",
        "no json here",
        "} backwards {",
        "{not json}",
        r#"["revised"]"#,
        r#"{"revised": "m3"}"#,
        r#"{"retired": {"id": "m3"}}"#,
        r#"{"summary": 3}"#,
    ] {
        assert!(decisions(answer, &given(), 120, 600).is_err(), "{answer}");
    }
}
