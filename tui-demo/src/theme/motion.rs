//! 压缩那一行动起来用的颜色（蓝图 `tui.md`「正文」第 9 条）：进度条最前面那一格明暗来回，整行往白里呼吸。
//! 都从主题的 `accent` 算，和运行状态行的流光一个做法。

use ratatui::style::{Color, Style};

use super::accent_rgb;

/// 进度条最前面亮着的那一格：`k` 从 0 到 1，从强调色的 `low` 成暗到往白里偏 `lift`。
pub fn frontier(k: f64, low: f64, lift: f64) -> Style {
    let (r, g, b) = accent_rgb();
    let k = k.clamp(0.0, 1.0);
    let at = |c: u8| {
        let dark = f64::from(c) * low;
        let light = f64::from(c) + (255.0 - f64::from(c)) * lift;
        (dark + (light - dark) * k).round().clamp(0.0, 255.0) as u8
    };
    Style::new().fg(Color::Rgb(at(r), at(g), at(b)))
}

/// 往白里偏 `amount`（0 到 1）：呼吸用。不是真彩色的前景照旧（色深统一换色在画完以后）。
pub fn lifted(style: Style, amount: f64) -> Style {
    match style.fg {
        Some(Color::Rgb(r, g, b)) if amount > 0.0 => {
            let up = |c: u8| {
                let c = f64::from(c);
                (c + (255.0 - c) * amount.min(1.0)).round() as u8
            };
            style.fg(Color::Rgb(up(r), up(g), up(b)))
        }
        _ => style,
    }
}

#[cfg(test)]
mod tests {
    use ratatui::style::{Color, Style};

    use super::{frontier, lifted};

    #[test]
    fn the_frontier_swings_between_dark_and_light_and_lifting_brightens() {
        let _theme = crate::theme::hold();
        let fg = |s: Style| match s.fg {
            Some(Color::Rgb(r, g, b)) => u32::from(r) + u32::from(g) + u32::from(b),
            _ => 0,
        };
        assert!(fg(frontier(0.0, 0.45, 0.35)) < fg(frontier(1.0, 0.45, 0.35)));
        let base = Style::new().fg(Color::Rgb(100, 100, 100));
        assert!(fg(lifted(base, 0.2)) > fg(base));
        assert_eq!(lifted(base, 0.0), base, "不呼吸不变");
        let named = Style::new().fg(Color::Blue);
        assert_eq!(lifted(named, 0.5), named, "不是真彩色的照旧");
    }
}
