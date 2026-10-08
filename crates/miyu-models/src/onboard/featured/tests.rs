//! `featured.toml` 的写法（施工 8-11 再补）：照先后读、名字一句字或者语言表、中文挑国内的编号；写错了说是第几家。

use super::*;

#[test]
fn providers_read_in_order_and_chinese_picks_the_domestic_id() {
    let text = "[[providers]]\ncatalog = \"moonshotai\"\ncatalog_zh = \"moonshotai-cn\"\nname = \"Kimi\"\n\n\
                [[providers]]\ncatalog = \"alibaba\"\nname = { zh = \"通义千问\", en = \"Qwen\" }\n";
    let read = featured(text).expect("读得进");
    assert_eq!(
        read.iter()
            .map(|one| one.catalog.as_str())
            .collect::<Vec<_>>(),
        ["moonshotai", "alibaba"]
    );
    assert_eq!(
        (read[0].id("zh"), read[0].id("en"), read[0].id("ja")),
        ("moonshotai-cn", "moonshotai", "moonshotai")
    );
    assert_eq!((read[1].id("zh"), read[1].id("en")), ("alibaba", "alibaba"));
    assert_eq!(
        (read[1].name.pick("zh"), read[1].name.pick("ja")),
        (Some("通义千问"), Some("Qwen"))
    );
}

#[test]
fn a_wrong_entry_says_which_one() {
    for (text, wanted) in [
        (
            "[[providers]]\ncatalog = \"a\"\nname = \"A\"\n[[providers]]\nname = \"B\"\n",
            "provider 2: catalog must be text",
        ),
        (
            "[[providers]]\ncatalog = 1\nname = \"A\"\n",
            "provider 1: catalog must be text",
        ),
        (
            "[[providers]]\ncatalog = \"a\"\n",
            "provider 1: name is missing",
        ),
        (
            "[[providers]]\ncatalog = \"a\"\nname = 3\n",
            "provider 1: name must be text or a language table",
        ),
        ("providers = 1\n", "no [[providers]]"),
        ("", "no [[providers]]"),
        ("[[providers]\n", "featured.toml: "),
    ] {
        let error = featured(text).expect_err(text);
        assert!(error.contains(wanted), "{text:?}：{error}");
    }
}
