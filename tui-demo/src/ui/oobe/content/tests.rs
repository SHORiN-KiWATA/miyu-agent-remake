//! 引导右边那一块的一行行（`content.rs`）。

use ratatui::text::Span;

use super::Content;

fn plain(c: &Content) -> Vec<String> {
    c.lines
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .map(|t| t.trim_end().to_string())
        .collect()
}

#[test]
fn a_long_list_opens_a_window_around_the_cursor() {
    // 2026-10-09 项目主人报：试通了列出 89 个模型，列表不会滚，选中的那一行走出了屏幕。
    let rows = |n: usize| (0..n).map(|i| vec![Span::raw(format!("m{i}"))]).collect();
    let mut c = Content::sized(30, 12);
    c.heading("标题", "");
    c.window(rows(89), 50, 0, ("↑ 还有 {n} 个", "↓ 还有 {n} 个"));
    let lines = plain(&c);
    assert!(c.len() <= 12, "放得下：{lines:?}");
    assert!(lines.iter().any(|l| l == "❯ m50"), "选中的露着：{lines:?}");
    assert!(
        lines.iter().any(|l| l.starts_with("  ↑ 还有 ")),
        "{lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.starts_with("  ↓ 还有 ")),
        "{lines:?}"
    );
    // 放得下的不开窗。
    let mut short = Content::sized(30, 12);
    short.window(rows(3), 0, 0, ("↑ {n}", "↓ {n}"));
    assert_eq!(
        plain(&short),
        ["❯ m0", "m1", "m2"].map(|s| if s.starts_with('❯') {
            s.to_string()
        } else {
            format!("  {s}")
        })
    );
}

#[test]
fn forms_point_and_color_while_lists_point_and_fill() {
    // 「第一次打开的引导」第 7 条（2026-10-10 项目主人：整行铺底「被文本框背景色盖住了」，「改成箭头加变颜色吧，
    // 列表里面可以是箭头加背景」）；停着的格子右端写「Enter 编辑」，编辑时换成光标。
    use crate::oobe::field::Field;
    use crate::theme;
    let accent = theme::accent().fg;
    let pointed = |c: &Content, i: usize| {
        let line = &c.lines[i];
        line.spans[0].content.starts_with('❯') && line.spans[0].style.fg == accent
    };
    let mut c = Content::new(40);
    c.cue = "Enter 编辑".into();
    let key = Field::line().with("sk-1");
    c.field("密钥", 4, &key, "", true);
    c.chip("提示词", 4, "", "写点什么", true);
    c.switch("接口", 4, "OpenAI 兼容", true);
    c.action(true, "测试连接 →");
    c.action(false, "下一步 →");
    c.row(true, vec![Span::raw("DeepSeek")], None);
    for i in 0..4 {
        assert!(pointed(&c, i), "第 {i} 行：{:?}", c.lines[i]);
        assert!(
            c.lines[i].spans[0].style.bg.is_none(),
            "填的格子、动作行不铺底：{:?}",
            c.lines[i]
        );
        assert_eq!(c.lines[i].spans[1].style.fg, accent, "名字变强调色");
    }
    assert!(!c.lines[4].spans[0].content.starts_with('❯'));
    assert_ne!(
        c.lines[4].spans[1].style.fg, accent,
        "没停上去的动作行照常色"
    );
    assert!(
        pointed(&c, 5) && c.lines[5].spans[0].style.bg == theme::row_focus().bg,
        "列表铺底"
    );
    let text = plain(&c);
    assert!(text[0].ends_with("Enter 编辑"), "{text:?}");
    assert!(text[1].ends_with("Enter 编辑"), "{text:?}");
    let mut editing = Content::new(40);
    editing.cue = "Enter 编辑".into();
    let mut key = Field::line().with("sk-1");
    key.begin();
    editing.field("密钥", 4, &key, "", true);
    assert!(!plain(&editing)[0].contains("Enter 编辑"));
    assert!(editing.caret.is_some());
}

#[test]
fn the_model_search_says_slash_and_is_focused_only_while_searching() {
    // 2026-10-10 项目主人：搜模型那一格写着「Enter 编辑」，「这里应该是 / 搜索吧」。
    use crate::oobe::field::Field;
    let mut c = Content::new(40);
    c.cue = "Enter 编辑".into();
    let mut filter = Field::line();
    c.search("搜索模型", 8, &filter, "/ 搜索");
    let text = plain(&c);
    assert!(
        text[0].ends_with("/ 搜索") && !text[0].starts_with('❯'),
        "{text:?}"
    );
    filter.begin();
    let mut c = Content::new(40);
    c.search("搜索模型", 8, &filter, "/ 搜索");
    assert!(plain(&c)[0].starts_with('❯') && c.caret.is_some());
    assert!(!plain(&c)[0].contains("/ 搜索"));
}
