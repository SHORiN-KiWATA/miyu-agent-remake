//! 编号表照「样子」对齐、选不了的不编号且是灰的、最后一行固定的编号；敲的编号、`0`、直接回车、读到头、超出的、别的字。

use super::*;

fn lines(rows: &[Row], last: Option<(&str, &str)>) -> Vec<String> {
    numbered(rows, last)
        .iter()
        .map(|line| line.paint(false))
        .collect()
}

fn cells(texts: &[&str]) -> Vec<String> {
    texts.iter().map(|text| (*text).to_string()).collect()
}

#[test]
fn the_found_table_follows_the_sample() {
    let rows = [
        Row::usable(cells(&["DeepSeek", "环境变量 DEEPSEEK_API_KEY"])),
        Row::usable(cells(&[
            "LMStudio",
            "本机 http://127.0.0.1:1234/v1，1 个模型",
        ])),
        Row {
            cells: cells(&[
                "Anthropic",
                "环境变量 ANTHROPIC_API_KEY，用不了：还没有 anthropic 驱动",
            ]),
            usable: false,
        },
    ];
    assert_eq!(
        lines(&rows, Some(("0", "都不要，从目录里找一家"))),
        [
            "  1  DeepSeek   环境变量 DEEPSEEK_API_KEY\n",
            "  2  LMStudio   本机 http://127.0.0.1:1234/v1，1 个模型\n",
            "     Anthropic  环境变量 ANTHROPIC_API_KEY，用不了：还没有 anthropic 驱动\n",
            "  0  都不要，从目录里找一家\n",
        ]
    );
    let painted = numbered(&rows, None)[2].paint(true);
    assert!(
        painted.starts_with("\u{1b}["),
        "用不了的是灰的：{painted:?}"
    );
    assert!(!numbered(&rows, None)[0].paint(true).starts_with("\u{1b}["));
}

#[test]
fn the_search_table_leaves_the_empty_note_out() {
    let rows = [
        Row::usable(cells(&["DeepSeek", "deepseek", ""])),
        Row {
            cells: cells(&["Deep Infra", "deepinfra", "用不了：认不出它的接口"]),
            usable: false,
        },
        Row::usable(cells(&["Ollama", "ollama", ""])),
    ];
    assert_eq!(
        lines(&rows, None),
        [
            "  1  DeepSeek    deepseek\n",
            "     Deep Infra  deepinfra  用不了：认不出它的接口\n",
            "  2  Ollama      ollama\n",
        ],
        "选不了的不占编号"
    );
}

#[test]
fn what_is_typed() {
    let line = |text: &str| Some(text.to_string());
    assert_eq!(typed(None, 3), Typed::End);
    assert_eq!(typed(line(""), 3), Typed::Empty);
    assert_eq!(typed(line("  \t"), 3), Typed::Empty);
    assert_eq!(typed(line("0"), 3), Typed::Zero);
    assert_eq!(typed(line(" 2 "), 3), Typed::Picked(1));
    assert_eq!(typed(line("3"), 3), Typed::Picked(2));
    assert_eq!(typed(line("4"), 3), Typed::Other("4".to_string()));
    assert_eq!(typed(line("1"), 0), Typed::Other("1".to_string()));
    assert_eq!(typed(line(" deep "), 3), Typed::Other("deep".to_string()));
}

#[test]
fn catalog_ids_that_are_not_names_get_one() {
    assert_eq!(config_id("deepseek"), "deepseek");
    assert_eq!(config_id("opencode-go"), "opencode-go");
    assert_eq!(config_id("302ai"), "p-302ai");
    assert_eq!(config_id("wafer.ai"), "wafer-ai");
    assert_eq!(config_id("Big_Lab"), "big_lab");
}
