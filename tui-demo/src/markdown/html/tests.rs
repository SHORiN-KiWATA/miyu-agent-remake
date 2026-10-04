//! 回答里的 HTML：`<img>`、`<svg>`、`<details>`（蓝图 `tui.md`「她的回答：Markdown」第 12、15 条，
//! 「图片、公式和 mermaid 图」第 8、9 条）。

use super::super::{FigureKind, Kit, MdLine, render};
use crate::config::Config;

fn draw(text: &str, width: u16, flipped: &[usize]) -> Vec<MdLine> {
    let config = Config::builtin().unwrap();
    let kit = Kit {
        languages: &config.languages,
        math: &config.math,
        labels: &config.text.markdown,
    };
    render(text, width, &kit, flipped)
}

/// 每行：引子加内容，去掉行尾空白。
fn lines(text: &str, flipped: &[usize]) -> Vec<String> {
    draw(text, 80, flipped)
        .iter()
        .map(|l| {
            let lead: String = l.lead.iter().map(|s| s.content.as_ref()).collect();
            let text = format!("{lead}{}", super::super::inline::plain(&l.folded));
            text.trim_end().to_string()
        })
        .collect()
}

const FOLD: &str = "<details>\n<summary>点我展开</summary>\n\n里面的字。\n\n</details>\n\n后面";

#[test]
fn details_are_folded_until_flipped() {
    assert_eq!(lines(FOLD, &[]), ["▶ 点我展开", "", "后面"]);
    assert_eq!(
        lines(FOLD, &[0]),
        ["▼ 点我展开", "│ 里面的字。", "", "后面"]
    );
    let open = "<details open>\n<summary>默认展开</summary>\n\n内容\n</details>";
    assert_eq!(
        lines(open, &[]),
        ["▼ 默认展开", "│ 内容"],
        "写了 open 的默认展开"
    );
    assert_eq!(lines(open, &[0]), ["▶ 默认展开"], "点过就收起");
}

#[test]
fn the_title_row_is_clickable_and_its_marker_is_not_copied() {
    let drawn = draw(FOLD, 80, &[]);
    let title = &drawn[0];
    assert_eq!(title.details, Some(0), "点它开关第 0 个");
    assert_eq!(super::super::inline::plain(&title.folded), "点我展开");
    let lead: String = title.lead.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(lead, "▶ ", "记号是引子，复制时不带");
    assert!(drawn.iter().skip(1).all(|l| l.details.is_none()));
}

#[test]
fn nested_details_get_a_rail_each_and_count_in_order() {
    let text = "<details open>\n<summary>外</summary>\n\n外层的字\n\n<details>\n<summary>里</summary>\n\n里层内容\n\n</details>\n\n</details>";
    assert_eq!(lines(text, &[]), ["▼ 外", "│ 外层的字", "│", "│ ▶ 里"]);
    assert_eq!(
        lines(text, &[1]),
        ["▼ 外", "│ 外层的字", "│", "│ ▼ 里", "│ │ 里层内容"]
    );
    assert_eq!(lines(text, &[0]), ["▶ 外"], "外层收着，里面的都不画");
}

#[test]
fn details_without_a_summary_on_one_line_or_unfinished() {
    assert_eq!(
        lines("<details>\n\n内容\n\n</details>", &[]),
        ["▶ 详细信息"]
    );
    let one = "<details><summary>行内的</summary>一行里的内容</details>";
    assert_eq!(lines(one, &[]), ["▶ 行内的"]);
    assert_eq!(lines(one, &[0]), ["▼ 行内的", "│ 一行里的内容"]);
    // 还在流、`</details>` 没到：收着的照样不画里面的。
    assert_eq!(
        lines("<details>\n<summary>标题</summary>\n\n还在写的内容", &[]),
        ["▶ 标题"]
    );
}

#[test]
fn img_tags_read_like_markdown_images() {
    let inline = "带尺寸：<img src=\"https://picsum.photos/240/160\" alt=\"缩略图\" width=\"240\" height=\"160\"> 后面的字";
    assert_eq!(
        lines(inline, &[]),
        ["带尺寸：[图片: 缩略图] <https://picsum.photos/240/160> 后面的字"]
    );
    assert!(
        draw(inline, 80, &[]).iter().all(|l| l.figure.is_none()),
        "网上的不画"
    );
    let local = draw(
        "<img src=\"/tmp/a.png\" alt=\"本机\" width=\"120\">",
        80,
        &[],
    );
    let figure = local.iter().find_map(|l| l.figure.as_ref()).unwrap();
    assert_eq!(
        (
            figure.kind,
            figure.source.as_str(),
            figure.width,
            figure.height
        ),
        (FigureKind::Image, "/tmp/a.png", Some(120), None)
    );
    assert_eq!(
        lines("<img src=\"/tmp/a.png\" alt=\"本机\" width=\"120\">", &[])[0],
        "[图片: 本机] </tmp/a.png>"
    );
}

#[test]
fn a_finished_svg_is_a_figure_and_an_unfinished_one_is_code() {
    let svg = "<svg width=\"120\" height=\"80\">\n  <rect x=\"5\" y=\"5\" width=\"110\" height=\"70\" fill=\"#4fa3ff\" />\n</svg>";
    let drawn = draw(&format!("{svg}\n\n完。"), 80, &[]);
    let figure = drawn[0].figure.as_ref().unwrap();
    assert_eq!(figure.kind, FigureKind::Svg);
    assert_eq!(figure.source.trim_end(), svg);
    let fallback: Vec<String> = figure
        .fallback
        .iter()
        .map(|l| super::super::inline::plain(&l.folded))
        .collect();
    assert!(
        fallback[0].starts_with("╭─ code svg"),
        "画不出来写源码：{fallback:?}"
    );
    assert_eq!(
        lines(&format!("{svg}\n\n完。"), &[]).last().unwrap(),
        "完。"
    );
    // 还在流：照代码写，不画半截。
    let open = lines("<svg width=\"120\">\n  <rect/>", &[]);
    assert!(open[0].starts_with("╭─ code svg"), "{open:?}");
    assert!(open.iter().any(|l| l == "  <rect/>"));
    // 一行里的：前后的字留着。
    let inline = "前面 <svg width=\"10\" height=\"10\"><rect/></svg> 后面";
    assert!(draw(inline, 80, &[]).iter().any(|l| l.figure.is_some()));
    let text = lines(inline, &[]).join("|");
    assert!(text.contains("前面") && text.contains("后面"), "{text}");
}

#[test]
fn common_inline_tags_read_like_markdown() {
    use ratatui::style::Modifier;
    assert_eq!(
        lines("按 <kbd>Ctrl</kbd>+<kbd>C</kbd> 复制，<b>很重要</b>", &[]),
        ["按 Ctrl+C 复制，很重要"]
    );
    let drawn = draw(
        "<b>粗</b><strong>重</strong><i>斜</i><em>强</em><u>下</u><s>删</s><del>去</del><code>码</code><mark>亮</mark>",
        80,
        &[],
    );
    let style = |text: &str| {
        drawn[0]
            .folded
            .spans
            .iter()
            .find(|s| s.content.contains(text))
            .map(|s| s.style)
            .unwrap()
    };
    assert_eq!(style("粗"), crate::theme::md_bold());
    assert_eq!(style("重"), crate::theme::md_bold());
    assert_eq!(style("斜"), crate::theme::md_italic());
    assert_eq!(style("强"), crate::theme::md_italic());
    assert!(style("下").add_modifier.contains(Modifier::UNDERLINED));
    assert!(style("删").add_modifier.contains(Modifier::CROSSED_OUT));
    assert!(style("去").add_modifier.contains(Modifier::CROSSED_OUT));
    assert_eq!(style("码"), crate::theme::md_code());
    assert_eq!(style("亮"), crate::theme::md_mark());
    assert_eq!(lines("H<sub>2</sub>O、x<sup>2</sup>", &[]), ["H₂O、x²"]);
    assert_eq!(lines("<sup>中</sup>", &[]), ["中"], "转不动的照原样");
    let link = draw("<a href=\"https://x.dev\">文档</a>", 80, &[]);
    assert_eq!(
        super::super::inline::plain(&link[0].folded),
        "文档 <https://x.dev>"
    );
    assert!(
        link[0]
            .folded
            .links
            .iter()
            .any(|(_, _, url)| url == "https://x.dev")
    );
}

#[test]
fn wrappers_are_dropped_hr_is_a_rule_and_open_tags_stop_at_the_block() {
    assert_eq!(
        lines("<div>\n不居中的字\n</div>\n\n后面", &[]),
        ["不居中的字", "", "后面"]
    );
    assert_eq!(
        lines("前面<span style=\"color:red\">红</span>后面", &[]),
        ["前面红后面"]
    );
    assert_eq!(
        lines("上\n\n<hr>\n\n下", &[]),
        lines("上\n\n---\n\n下", &[])
    );
    // 没收尾的 `<b>` 到这一段结尾为止，下一段不粗。
    let drawn = draw("<b>没收尾\n\n下一段", 80, &[]);
    let last = drawn.last().unwrap();
    assert!(
        last.folded
            .spans
            .iter()
            .all(|s| s.style != crate::theme::md_bold())
    );
}

#[test]
fn other_tags_are_still_written_as_they_are() {
    assert_eq!(
        lines("按 <abbr>Ctrl</abbr> 键", &[]),
        ["按 <abbr>Ctrl</abbr> 键"]
    );
    assert_eq!(
        lines("<table><tr><td>格</td></tr></table>", &[]),
        ["<table><tr><td>格</td></tr></table>"]
    );
    assert_eq!(lines("上<br>下", &[]), ["上", "下"]);
}

/// 量尺：一段两万字上下的回答整段重排一次要多久，纯 Markdown 和夹着一堆 HTML 的比
/// （流式时正在写的那一条每帧重排一次；`cargo test html_ruler -- --ignored --nocapture`）。
#[test]
#[ignore = "量尺：看耗时用"]
fn html_ruler() {
    let plain: String = (0..120)
        .map(|i| format!("## 第 {i} 节\n\n这是一段说明文字，**粗体** 和 `代码`，写长一点让它折行折行折行折行折行折行折行。\n\n- 一项\n- 两项\n\n"))
        .collect();
    let html: String = (0..120)
        .map(|i| format!("<details open>\n<summary>第 {i} 节</summary>\n\n这是一段说明文字，<b>粗体</b> 和 <kbd>Ctrl</kbd>，写长一点让它折行折行折行折行折行折行折行。<br>换一行\n\n- 一项\n- 两项\n\n</details>\n<img src=\"https://x/{i}.png\" alt=\"图\" width=\"40\">\n\n"))
        .collect();
    let config = Config::builtin().unwrap();
    let kit = Kit {
        languages: &config.languages,
        math: &config.math,
        labels: &config.text.markdown,
    };
    let time = |text: &str| {
        let start = std::time::Instant::now();
        for _ in 0..200 {
            std::hint::black_box(render(text, 80, &kit, &[]));
        }
        start.elapsed() / 200
    };
    for (name, text) in [("纯 Markdown", &plain), ("夹 HTML", &html)] {
        let took = time(text);
        let chars = text.chars().count();
        let per = took.as_secs_f64() * 1e9 / chars as f64;
        println!("{name}：{chars} 字，{took:?}/次，每字 {per:.0}ns");
    }
}

#[test]
fn center_and_align_center_are_centered_in_the_room() {
    // 2026-09-30 项目主人要做：`<center>`、写了 `align="center"` 的外壳，里面的每一行在正文宽度里居中（左边补空格，
    // 复制时不带）。
    let pad =
        |text: &str| " ".repeat((80 - unicode_width::UnicodeWidthStr::width(text)) / 2) + text;
    assert_eq!(lines("<center>居中</center>", &[]), [pad("居中")]);
    assert_eq!(
        lines("<div align=\"center\">\n居中的字\n</div>\n\n后面", &[]),
        [pad("居中的字"), String::new(), "后面".into()]
    );
    assert_eq!(lines("<p align='CENTER'>标题</p>", &[]), [pad("标题")]);
    // 居中补的空格在引子里：复制时不带。
    let drawn = draw("<center>居中</center>", 80, &[]);
    assert_eq!(super::super::inline::plain(&drawn[0].folded), "居中");
}
