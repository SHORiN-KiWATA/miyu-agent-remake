//! mermaid 图由核心画（核心 W-4 `mermaid.render`，图纸 `mermaid.md`）：给源码，交回 SVG 和三种记号色；界面照主题换色再
//! 栅格化（蓝图 `tui.md`「图片、公式和 mermaid 图」第 4 条）。

use serde_json::Value;

/// 核心画好的一张图。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    /// SVG 的字。
    pub svg: String,
    /// 图里填的三种记号色。
    pub marks: Marks,
}

/// SVG 里字、线、连线标签垫底先填的记号色（`#rrggbb`），界面换成主题色。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marks {
    /// 连线标签垫的底。
    pub label: String,
    /// 框线、连线。
    pub line: String,
    /// 字。
    pub text: String,
}

/// `mermaid.render` 的回应；缺 SVG、缺记号色的是 `None`（照画不出，写源码）。
pub fn read(result: &Value) -> Option<Rendered> {
    let text = |v: &Value| v.as_str().filter(|s| !s.is_empty()).map(str::to_string);
    let marks = &result["marks"];
    Some(Rendered {
        svg: text(&result["svg"])?,
        marks: Marks {
            label: text(&marks["label"])?,
            line: text(&marks["line"])?,
            text: text(&marks["text"])?,
        },
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::read;

    #[test]
    fn a_reply_reads_its_svg_and_marks_and_a_short_one_is_none() {
        let got = read(
            &json!({"marks": {"label": "#070809", "line": "#040506", "text": "#010203"},
            "svg": "<svg/>"}),
        )
        .unwrap();
        assert_eq!(got.svg, "<svg/>");
        assert_eq!(
            (
                got.marks.label.as_str(),
                got.marks.line.as_str(),
                got.marks.text.as_str()
            ),
            ("#070809", "#040506", "#010203")
        );
        assert_eq!(read(&json!({"svg": "<svg/>"})), None);
        assert_eq!(
            read(&json!({"marks": {"label": "#1", "line": "#2", "text": "#3"}})),
            None
        );
    }
}
