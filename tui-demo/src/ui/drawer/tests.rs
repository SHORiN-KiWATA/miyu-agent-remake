//! 画抽屉（蓝图「确认和提问的抽屉」第 2、3 条）：谁在问、标签、选中的那一项、编辑的光标、补充、「确认」页、
//! 文字画的框、放不下时钉住问题和按键提示。

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use unicode_width::UnicodeWidthStr;

use super::{fit, view};
use crate::config::Config;
use crate::drawer::{Asked, Drawer, Texts};

fn text(line: &ratatui::text::Line) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

fn lines(d: &Drawer, width: u16) -> Vec<String> {
    view(d, &texts(), &look(), width)
        .lines
        .iter()
        .map(text)
        .collect()
}

fn look() -> super::PreviewLook {
    Config::builtin().unwrap().layout.drawer_preview
}

fn texts() -> Texts {
    Config::builtin().unwrap().text.drawer
}

fn press(d: &mut Drawer, code: KeyCode) {
    d.key(KeyEvent::new(code, KeyModifiers::NONE));
}

/// 两道题：第一道 `n` 个选项（带说明），第二道没有短名。
fn asked(n: usize) -> Asked {
    let options: Vec<_> = (0..n)
        .map(|i| serde_json::json!({"label": format!("选项{i}"), "description": "说明"}))
        .collect();
    serde_json::from_value(serde_json::json!({
        "call_id": "c",
        "questions": [
            {"header": "甲", "question": "第一道？", "options": options},
            {"question": "第二道？"}
        ]
    }))
    .unwrap()
}

#[test]
fn it_shows_who_asks_the_tabs_the_pointer_and_the_keys() {
    let d = Drawer::question(Some("子代理 查资料".into()), asked(2));
    let v = view(&d, &texts(), &look(), 60);
    let got: Vec<String> = v.lines.iter().map(text).collect();
    assert_eq!(got[0], "子代理 查资料 在问");
    assert_eq!(
        got[1], " 甲   第二道？  确认",
        "当前那个反色（两边空一格），没有短名的写问题，最后是「确认」"
    );
    assert_eq!(got[3], "第一道？");
    assert_eq!(got[5], "› 选项0", "选中的行首 ›");
    assert_eq!(got[6], "  说明", "说明和标题对齐");
    assert_eq!(got[7], "  选项1");
    assert_eq!(
        got[9], "  输入其他答案",
        "最后一项是「输入其他答案」，没有「不回答」"
    );
    assert_eq!(
        got.last().unwrap(),
        "↑/↓ 选择 · Enter 提交 · n 补充 · Esc ×2 取消 · ←/→ 换题"
    );
    assert_eq!(v.focus, (5, 6));
    assert_eq!((v.items[5], v.items[3]), (Some(0), None));
    // 标题加粗、选中的品红。
    let label = &v.lines[5].spans[1];
    assert!(
        label
            .style
            .add_modifier
            .contains(ratatui::style::Modifier::BOLD)
    );
    assert_eq!(label.style.fg, crate::theme::picked().fg);
}

#[test]
fn other_edits_in_place_and_the_keys_say_only_save_and_leave() {
    let mut d = Drawer::question(None, asked(1));
    press(&mut d, KeyCode::Down);
    press(&mut d, KeyCode::Enter);
    let v = view(&d, &texts(), &look(), 60);
    let (row, col) = v.cursor.expect("编辑时有光标");
    assert_eq!(
        text(&v.lines[row]),
        "› 输入其他答案",
        "还没打字：暗色写提示"
    );
    assert_eq!(col, 2, "光标落在 › 后面");
    assert_eq!(text(v.lines.last().unwrap()), "Enter 保存 · Esc 退出编辑");
    press(&mut d, KeyCode::Char('好'));
    let v = view(&d, &texts(), &look(), 60);
    let (row, col) = v.cursor.unwrap();
    assert_eq!((text(&v.lines[row]).as_str(), col), ("› 好", 4));
    // 退出编辑：写成「自定义：好」。
    press(&mut d, KeyCode::Esc);
    assert!(lines(&d, 60).contains(&"› 自定义：好".to_string()));
}

#[test]
fn notes_show_under_the_options() {
    let mut d = Drawer::question(None, asked(1));
    press(&mut d, KeyCode::Char('n'));
    for c in "快".chars() {
        press(&mut d, KeyCode::Char(c));
    }
    let v = view(&d, &texts(), &look(), 60);
    let (row, _) = v.cursor.unwrap();
    assert_eq!(text(&v.lines[row]), "  补充  快");
}

#[test]
fn the_review_page_lists_every_answer() {
    let mut d = Drawer::question(None, asked(1));
    press(&mut d, KeyCode::Enter);
    press(&mut d, KeyCode::Right);
    assert!(d.on_review());
    let got = lines(&d, 60);
    assert_eq!(got[0], "甲 ✓  第二道？   确认 ", "「确认」反色，答过的带 ✓");
    assert_eq!(got[2], "甲：选项0");
    assert_eq!(got[3], "第二道？：未回答");
    assert_eq!(got.last().unwrap(), "Enter 提交 · Esc ×2 取消 · ←/→ 换题");
}

#[test]
fn a_preview_sits_on_the_right_under_a_dim_title_and_below_when_narrow() {
    let asked: Asked = serde_json::from_value(serde_json::json!({
        "call_id": "c",
        "questions": [{"question": "哪种？", "options": [
            {"label": "左右", "preview": "┌──┬──┐\n│甲│乙│\n└──┴──┘"},
            {"label": "上下", "preview": "┌──┐\n└──┘"}
        ]}]
    }))
    .unwrap();
    let d = Drawer::question(None, asked);
    let wide = lines(&d, 100);
    // 竖线在中线（第 50 列），预览从第 52 列起，和选项顶上对齐：第一行暗色「预览」，下面是图；不铺底色，中间一条竖线（2026-10-07 项目主人）。
    assert!(wide[2].starts_with("› 左右"), "{wide:?}");
    let title = wide[2].find("预览").expect("第一行写预览");
    assert_eq!(wide[2][..title].width(), 52);
    let at = wide[3].find('┌').expect("图在下一行");
    assert_eq!(wide[3][..at].width(), 52);
    assert!(wide[5].contains("└──┴──┘"));
    assert!(!wide.iter().any(|l| l.contains('╭')), "不描边");
    for row in &wide[2..6] {
        let bar = row.find('│').expect("每行都有竖线");
        assert_eq!(row[..bar].width(), 50, "竖线在抽屉的中线上：{row}");
    }
    let v = view(&d, &texts(), &look(), 100);
    assert!(
        v.lines[3].spans.iter().all(|s| s.style.bg.is_none()),
        "不铺底色：{:?}",
        v.lines[3]
    );
    // 窄了：挪到选项下面，中间空一行。
    let narrow = lines(&d, 50);
    assert_eq!(narrow[2], "› 左右");
    let title = narrow.iter().position(|l| l == "预览").expect("有预览");
    assert_eq!(narrow[title - 1], "", "{narrow:?}");
    assert_eq!(narrow[title + 1], "┌──┬──┐", "{narrow:?}");
    assert!(!narrow[2].contains('│'), "上下排不画竖线");
}

#[test]
fn a_tall_drawer_scrolls_the_options_and_pins_the_question_and_keys() {
    let mut d = Drawer::question(None, asked(12));
    for _ in 0..12 {
        press(&mut d, KeyCode::Down);
    }
    let full = view(&d, &texts(), &look(), 60);
    let keys = text(full.lines.last().unwrap());
    let mut scroll = 0;
    let v = fit(full, 10, &mut scroll);
    let got: Vec<String> = v.lines.iter().map(text).collect();
    assert_eq!(got.len(), 10);
    assert_eq!(got[9], keys, "按键提示钉在最下面");
    assert_eq!(got[2], "第一道？", "问题钉在上面，不滚走");
    assert!(
        got.contains(&"› 输入其他答案".to_string()),
        "选中的那一项露出来"
    );
    assert!(scroll > 0);
}

#[test]
fn a_very_short_drawer_keeps_the_picked_option_before_the_blank_line() {
    // 2026-09-29 30×7 实测：抽屉只剩问题的最后一行、空行、按键提示，选项一行都没有（「窗口小的时候」第 3 条）。
    let d = Drawer::question(None, asked(4));
    let full = view(&d, &texts(), &look(), 60);
    let keys = text(full.lines.last().unwrap());
    let picked = full
        .lines
        .iter()
        .map(text)
        .find(|l| l.starts_with("› "))
        .unwrap();
    let fitted = |height: usize| -> Vec<String> {
        let mut scroll = 0;
        fit(view(&d, &texts(), &look(), 60), height, &mut scroll)
            .lines
            .iter()
            .map(text)
            .collect()
    };
    let three = fitted(3);
    assert_eq!(three.len(), 3);
    assert_eq!(three[1], picked, "先去空行，留一行选项：{three:?}");
    assert_eq!(three[2], keys);
    assert_eq!(fitted(2), [picked.clone(), keys], "再去问题");
    assert_eq!(
        fitted(1),
        std::slice::from_ref(&picked),
        "最后连按键提示也去掉"
    );
    // 四行起照旧：问题、选项、空行、按键提示。
    let four = fitted(4);
    assert_eq!((four[1].as_str(), four[2].as_str()), (picked.as_str(), ""));
}

#[test]
fn the_first_esc_turns_the_keys_line_into_a_warning() {
    use std::time::{Duration, Instant};
    let mut d = Drawer::question(None, asked(1));
    d.escape(Instant::now(), Duration::from_secs(60));
    let v = view(&d, &texts(), &look(), 60);
    let last = v.lines.last().unwrap();
    assert_eq!(text(last), "再按一次 Esc 取消");
    assert_eq!(last.style.fg, crate::theme::warn().fg);
}
