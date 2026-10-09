//! 能出向量的模型（施工 R-5 再补，`docs/blueprint/models.md`「协议」`model.list` 的「能出向量的」）：照资源目录的
//! `models/embedding.toml` 认，`model.list` 里认得出的模型多一格 `embedding: true`，别的不写这一格。设置页的「语义模型」照它挑。

use serde_json::{Value, json};

use crate::support::*;

#[tokio::test]
async fn embedding_models_are_marked_in_the_list() {
    let home = Home::new();
    home.write(
        "system/config.toml",
        "[providers.dev]\ndriver = \"openai-chat\"\nbase_url = \"https://dev.example.invalid/v1\"\n\n\
         [providers.dev.models.\"text-embedding-3-small\"]\nwindow = 8192\n\n\
         [providers.dev.models.\"BAAI/bge-m3\"]\nwindow = 8192\n\n\
         [providers.dev.models.\"bge-reranker-v2-m3\"]\nwindow = 8192\n\n\
         [providers.dev.models.\"gpt-4o\"]\nwindow = 8192\n",
    );
    let data = providers::data(providers::profiles(json!({})));
    let mut client = Client::connect(providers::core(&home, &[], data));
    client.hello().await;
    let reply = client.call("c1", "model.list", json!({})).await;
    let models = reply["result"]["providers"][0]["models"]
        .as_array()
        .unwrap_or_else(|| panic!("有模型：{reply}"));
    let marked: Vec<(&str, &Value)> = models
        .iter()
        .map(|model| {
            (
                model["model"].as_str().unwrap_or_default(),
                &model["embedding"],
            )
        })
        .collect();
    assert_eq!(
        marked,
        [
            ("BAAI/bge-m3", &json!(true)),
            ("bge-reranker-v2-m3", &Value::Null),
            ("gpt-4o", &Value::Null),
            ("text-embedding-3-small", &json!(true)),
        ],
        "{reply}"
    );
    assert!(
        models
            .iter()
            .filter(|model| model["embedding"].is_null())
            .all(|model| model.get("embedding").is_none()),
        "不是的不写这一格"
    );
}

/// 名字认不出、目录里对上的条目的 `family` 认得出的也算：配置手写 `catalog` 对上一份手造的目录。
#[tokio::test]
async fn a_catalog_family_marks_it_too() {
    use miyu_models::catalog::{Catalog, CatalogSource, Loaded};
    use miyu_models::matching::Vendors;
    use miyu_session::{ModelData, Observed};

    let catalog = json!({"devcat": {"id": "devcat", "name": "Dev", "models": {
        "mini_lm_l12_v2": {"id": "mini_lm_l12_v2", "family": "text-embedding"},
        "chatty-1": {"id": "chatty-1", "family": "chatty"},
    }}});
    let data = ModelData::new(providers::profiles(json!({})), Vendors::default(), None);
    data.loaded(
        Some(Loaded {
            catalog: Catalog::parse(&catalog.to_string())
                .expect("读得进")
                .catalog,
            source: CatalogSource::Snapshot,
            fetched: "2026-10-09T00:00:00.000Z".to_string(),
        }),
        Observed::default(),
    );
    let home = Home::new();
    home.write(
        "system/config.toml",
        "[providers.dev]\ndriver = \"openai-chat\"\nbase_url = \"https://dev.example.invalid/v1\"\ncatalog = \"devcat\"\n",
    );
    let mut client = Client::connect(providers::core(&home, &[], std::sync::Arc::new(data)));
    client.hello().await;
    let reply = client.call("c1", "model.list", json!({})).await;
    let models = reply["result"]["providers"][0]["models"]
        .as_array()
        .unwrap_or_else(|| panic!("有模型：{reply}"));
    let marked: Vec<(&str, &Value)> = models
        .iter()
        .map(|model| {
            (
                model["model"].as_str().unwrap_or_default(),
                &model["embedding"],
            )
        })
        .collect();
    assert_eq!(
        marked,
        [("chatty-1", &Value::Null), ("mini_lm_l12_v2", &json!(true))],
        "{reply}"
    );
}
