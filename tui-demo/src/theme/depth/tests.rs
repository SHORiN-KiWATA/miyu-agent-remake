//! 色深（蓝图 `tui.md`「主题」第 5、6 条）：照环境变量探测；画完一帧统一换色。

use std::collections::HashMap;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use super::{Depth, degrade, fit};

fn detect(vars: &[(&str, &str)]) -> Depth {
    let vars: HashMap<&str, &str> = vars.iter().copied().collect();
    Depth::detect(|name| vars.get(name).map(|v| (*v).to_string()))
}

#[test]
fn depth_follows_the_environment() {
    assert_eq!(
        detect(&[("COLORTERM", "truecolor"), ("TERM", "xterm-256color")]),
        Depth::True
    );
    assert_eq!(detect(&[("COLORTERM", "24bit")]), Depth::True);
    assert_eq!(
        detect(&[("TERM", "xterm-kitty")]),
        Depth::True,
        "kitty 没设 COLORTERM 也是真彩色"
    );
    assert_eq!(
        detect(&[("TERM", "xterm-256color"), ("TERM_PROGRAM", "iTerm.app")]),
        Depth::True
    );
    assert_eq!(
        detect(&[
            ("TERM", "xterm-256color"),
            ("TERM_PROGRAM", "Apple_Terminal")
        ]),
        Depth::X256,
        "macOS 自带的终端"
    );
    assert_eq!(detect(&[("TERM", "linux")]), Depth::Ansi16);
    assert_eq!(detect(&[("TERM", "dumb")]), Depth::Mono);
    assert_eq!(detect(&[]), Depth::Mono, "TERM 空");
}

#[test]
fn no_color_wins_and_miyu_color_overrides_the_guess() {
    assert_eq!(
        detect(&[
            ("NO_COLOR", "1"),
            ("MIYU_COLOR", "truecolor"),
            ("COLORTERM", "truecolor")
        ]),
        Depth::Mono
    );
    assert_eq!(
        detect(&[("MIYU_COLOR", "256"), ("COLORTERM", "truecolor")]),
        Depth::X256
    );
    assert_eq!(
        detect(&[("MIYU_COLOR", "16"), ("TERM", "xterm-kitty")]),
        Depth::Ansi16
    );
    assert_eq!(
        detect(&[("MIYU_COLOR", "none"), ("TERM", "xterm-kitty")]),
        Depth::Mono
    );
    assert_eq!(
        detect(&[("MIYU_COLOR", "TrueColor"), ("TERM", "linux")]),
        Depth::True
    );
    assert_eq!(
        detect(&[("MIYU_COLOR", "写错了"), ("TERM", "xterm-kitty")]),
        Depth::X256,
        "写错的当 256"
    );
}

#[test]
fn rgb_goes_to_the_nearest_of_the_cube_or_the_greys() {
    // 强调色 #7aa2f7：色立方里的 #87afff 更近。
    assert_eq!(
        fit(Color::Rgb(0x7a, 0xa2, 0xf7), Depth::X256),
        Color::Indexed(111)
    );
    // 选中的底色 #292e42：灰阶里的 #303030 比色立方的 #00005f 近。
    assert_eq!(
        fit(Color::Rgb(0x29, 0x2e, 0x42), Depth::X256),
        Color::Indexed(236)
    );
    // 正好在色立方上的原样对上。
    assert_eq!(
        fit(Color::Rgb(0xff, 0x87, 0x00), Depth::X256),
        Color::Indexed(208)
    );
    // 256 色、16 色的名字、终端自己的颜色不动。
    for kept in [Color::Indexed(73), Color::Red, Color::Reset] {
        assert_eq!(fit(kept, Depth::X256), kept);
    }
    assert_eq!(fit(Color::Rgb(1, 2, 3), Depth::True), Color::Rgb(1, 2, 3));
}

#[test]
fn sixteen_colours_use_the_names() {
    assert_eq!(
        fit(Color::Rgb(0xf7, 0x76, 0x8e), Depth::Ansi16),
        Color::LightRed
    );
    assert_eq!(
        fit(Color::Rgb(0x82, 0x8b, 0xb8), Depth::Ansi16),
        Color::Gray
    );
    assert_eq!(fit(Color::Indexed(236), Depth::Ansi16), Color::DarkGray);
    assert_eq!(
        fit(Color::Indexed(4), Depth::Ansi16),
        Color::Blue,
        "16 号以前的写成名字"
    );
    assert_eq!(fit(Color::Cyan, Depth::Ansi16), Color::Cyan);
}

#[test]
fn a_frame_is_repainted_once_at_the_end() {
    let area = Rect::new(0, 0, 3, 1);
    let style = Style::new()
        .fg(Color::Rgb(0x7a, 0xa2, 0xf7))
        .bg(Color::Rgb(0x29, 0x2e, 0x42))
        .add_modifier(Modifier::BOLD);
    let painted = || {
        let mut buf = Buffer::empty(area);
        buf.set_string(0, 0, "字a", style);
        buf
    };
    let mut buf = painted();
    degrade(&mut buf, Depth::True);
    assert_eq!(buf, painted(), "真彩色原样");
    degrade(&mut buf, Depth::X256);
    let cell = &buf[(0, 0)];
    assert_eq!(
        (cell.fg, cell.bg),
        (Color::Indexed(111), Color::Indexed(236))
    );
    let mut buf = painted();
    degrade(&mut buf, Depth::Mono);
    let cell = &buf[(2, 0)];
    assert_eq!(
        (cell.fg, cell.bg),
        (Color::Reset, Color::Reset),
        "不上色：前景背景都用终端自己的"
    );
    assert!(cell.modifier.contains(Modifier::BOLD), "修饰照留");
}

#[test]
fn the_cells_of_a_kitty_picture_keep_their_colour() {
    // 2026-09-30 项目主人带着 `MIYU_COLOR=256` 开，图的位置全是空的：kitty 靠格子的前景色认是哪张图。
    use image::{DynamicImage, Rgba, RgbaImage};
    use ratatui::layout::Size;
    use ratatui::widgets::Widget;
    use ratatui_image::Resize;
    use ratatui_image::picker::{Picker, ProtocolType};
    use ratatui_image::sliced::{SignedPosition, SlicedImage, SlicedProtocol};

    let mut picker = Picker::halfblocks();
    picker.set_protocol_type(ProtocolType::Kitty);
    let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(40, 40, Rgba([0, 128, 255, 255])));
    let protocol =
        SlicedProtocol::new_with_resize(&picker, image, Size::new(4, 2), Resize::Fit(None))
            .unwrap();
    let area = Rect::new(0, 0, 6, 2);
    let painted = || {
        let mut buf = Buffer::empty(area);
        buf.set_string(4, 0, "字", Style::new().fg(Color::Rgb(0x7a, 0xa2, 0xf7)));
        SlicedImage::new(&protocol, SignedPosition { x: 0, y: 0 }).render(area, &mut buf);
        buf
    };
    let picture: Vec<(u16, u16)> = area
        .positions()
        .filter(|p| painted()[(p.x, p.y)].symbol().starts_with('\u{10EEEE}'))
        .map(|p| (p.x, p.y))
        .collect();
    assert!(!picture.is_empty(), "画出了 kitty 的占位格");
    for depth in [Depth::X256, Depth::Ansi16, Depth::Mono] {
        let mut buf = painted();
        degrade(&mut buf, depth);
        for &at in &picture {
            assert_eq!(buf[at], painted()[at], "{depth:?}：图的格子不换色");
        }
        assert_ne!(buf[(4, 0)].fg, painted()[(4, 0)].fg, "{depth:?}：字照换");
    }
}
