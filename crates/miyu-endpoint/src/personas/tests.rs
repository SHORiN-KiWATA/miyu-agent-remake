//! 记忆归哪个账号、照语言挑名字（施工 P-1 上）。

use super::*;

fn found(home: Option<&str>) -> Found {
    Found {
        id: "miyu".to_string(),
        file: Default::default(),
        texts: Default::default(),
        layers: vec![Layer::Shipped],
        persona_from: None,
        examples_from: None,
        reminders_from: None,
        base: None,
        home: home.map(|account| AccountId::parse(account).unwrap()),
    }
}

#[test]
fn memory_follows_the_home_the_persona_lives_in() {
    let owner = AccountId::parse("alice").unwrap();
    assert_eq!(
        memory_account(&found(Some("admin")), &owner).as_str(),
        "admin"
    );
    assert_eq!(
        memory_account(&found(None), &owner).as_str(),
        "alice",
        "出厂、系统区的归属主"
    );
}

#[test]
fn names_are_picked_by_language_then_english_chinese_japanese() {
    let phrases: miyu_store::personas::Phrases = [("zh", "美羽"), ("ja", "ミユ")]
        .into_iter()
        .map(|(language, text)| (language.to_string(), text.to_string()))
        .collect();
    assert_eq!(pick(&phrases, "ja").as_deref(), Some("ミユ"));
    assert_eq!(
        pick(&phrases, "en").as_deref(),
        Some("美羽"),
        "没有英文的照中文"
    );
    let english: miyu_store::personas::Phrases = [("en".to_string(), "Miyu".to_string())]
        .into_iter()
        .collect();
    assert_eq!(pick(&english, "zh").as_deref(), Some("Miyu"));
    assert_eq!(pick(&Default::default(), "zh"), None);
}
