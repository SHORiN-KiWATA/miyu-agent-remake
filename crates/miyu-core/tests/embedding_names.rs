//! 出厂的认能出向量的规矩（施工 R-5 再补，`docs/blueprint/models.md`「协议」`model.list` 的「能出向量的」）照资源目录的目录快照
//! 认：几家常见的 embedding 模型认得出，重排的不算，认出来的没有一个能调工具（聊天模型一个不认）。

use miyu_models::catalog::Catalog;
use miyu_models::embedding::EmbeddingNames;

use crate::support::resources;

#[test]
fn the_shipped_rules_pick_embedding_models_from_the_snapshot() {
    let resources = resources();
    let names =
        EmbeddingNames::parse(&resources.embedding_names().expect("读得到")).expect("出厂的写法对");
    let text = std::fs::read_to_string(resources.catalog_snapshot()).expect("有快照");
    let catalog = Catalog::parse(&text).expect("读得进").catalog;
    let mut picked = Vec::new();
    for provider in catalog.providers() {
        for (model, entry) in &provider.models {
            if names.matches(model, entry.family.as_deref()) {
                picked.push((provider.id.clone(), model.clone(), entry.tools));
            }
        }
    }
    let has = |provider: &str, model: &str| {
        picked
            .iter()
            .any(|(p, m, _)| p == provider && **m == *model)
    };
    for (provider, model) in [
        ("openai", "text-embedding-3-small"),
        ("google", "gemini-embedding-001"),
        ("huggingface", "Qwen/Qwen3-Embedding-8B"),
        ("digitalocean", "bge-m3"),
        ("mistral", "mistral-embed"),
    ] {
        assert!(has(provider, model), "{provider}/{model}");
    }
    assert!(!has("digitalocean", "bge-reranker-v2-m3"), "重排的不算");
    let chatty: Vec<_> = picked
        .iter()
        .filter(|(_, _, tools)| *tools == Some(true))
        .collect();
    assert!(
        chatty.is_empty(),
        "认出来的能调工具，多半是聊天模型：{chatty:?}"
    );
    assert!(picked.len() >= 60, "认出来的太少：{}", picked.len());
}
