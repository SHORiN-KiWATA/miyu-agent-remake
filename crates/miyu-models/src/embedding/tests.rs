//! `embedding.toml` 的写法和认法（施工 R-5 再补）：名字、`family` 含哪几个字、名字最后一段和 `family` 以什么开头，`except` 压过；
//! 不分大小写；写错了说是哪一格。

use super::*;

fn shipped() -> EmbeddingNames {
    EmbeddingNames::parse("contains = [\"embed\"]\nstarts = [\"BGE\"]\nexcept = [\"rerank\"]\n")
        .expect("读得进")
}

#[test]
fn the_rules_read_lowercased_and_may_be_left_out() {
    assert_eq!(
        shipped(),
        EmbeddingNames {
            contains: vec!["embed".to_string()],
            starts: vec!["bge".to_string()],
            except: vec!["rerank".to_string()],
        }
    );
    assert_eq!(
        EmbeddingNames::parse("").expect("空的也读得进"),
        EmbeddingNames::default()
    );
}

#[test]
fn names_and_families_are_matched() {
    let names = shipped();
    for (model, family) in [
        ("text-embedding-3-small", Some("text-embedding")),
        ("Qwen/Qwen3-Embedding-8B", Some("qwen")),
        ("BAAI/bge-m3", None),
        ("bge-large-zh-v1.5", None),
        ("mini_lm_l12_v2", Some("text-embedding")),
        ("m3", Some("bge")),
    ] {
        assert!(names.matches(model, family), "{model} {family:?}");
    }
    for (model, family) in [
        ("gpt-4o", Some("gpt")),
        ("bge-reranker-v2-m3", Some("bge")),
        ("qwen3-reranker-8b", Some("qwen-embed")),
        ("deepseek-v4", None),
        // 开头那一条只看最后一段：供应商的前缀里有 bge 的不算。
        ("bgeco/chat-1", None),
        ("", None),
    ] {
        assert!(!names.matches(model, family), "{model} {family:?}");
    }
    assert!(
        !EmbeddingNames::default().matches("text-embedding-3-small", None),
        "没有规矩的一个都不认"
    );
}

#[test]
fn a_wrong_rule_says_where() {
    for (text, says) in [
        ("contains = [", "embedding.toml"),
        ("contains = \"embed\"\n", "contains"),
        ("starts = [1]\n", "starts"),
        ("except = [\"\"]\n", "except"),
        ("include = [\"x\"]\n", "include"),
    ] {
        let error = EmbeddingNames::parse(text).expect_err(text);
        assert!(error.contains(says), "{text:?}：{error}");
    }
}
