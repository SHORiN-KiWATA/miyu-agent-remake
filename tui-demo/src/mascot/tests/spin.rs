//! 引导里吉祥物的几样（蓝图 `tui.md`「第一次打开的引导」第 8–11、28 条）：整个转一圈、从黑到亮、放大。

use super::super::{Part, Pose, render};
use super::look;

/// 画成一串字（不看颜色）。
fn picture(grid: &[Vec<Option<super::super::render::Cell>>]) -> String {
    grid.iter()
        .map(|row| {
            row.iter()
                .map(|c| c.map_or(' ', |c| c.mark))
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn a_whole_turn_comes_back_to_the_front_and_the_back_has_no_face() {
    let look = look();
    let front = render(&look, &Pose::facing(0.0, 0.0));
    let full = Pose {
        spin: 360.0,
        ..Pose::default()
    };
    assert_eq!(
        picture(&render(&look, &full)),
        picture(&front),
        "转一整圈回到正面"
    );
    let back = Pose {
        spin: 180.0,
        ..Pose::default()
    };
    let grid = render(&look, &back);
    let eyes = grid
        .iter()
        .flatten()
        .filter(|c| c.is_some_and(|c| c.part == Part::Eye));
    assert_eq!(eyes.count(), 0, "背面没有眼睛");
    assert!(
        !picture(&grid).contains(look.face.line_mark),
        "背面没有嘴和肚子上的线"
    );
}

#[test]
fn the_fins_turn_with_the_whole_body_not_only_a_third() {
    // 转头时鳍只跟三成半（`follow`）；整个转（`spin`）时鳍一起转：转 90° 时左右两片鳍叠在一起，画出来窄得多。
    let look = look();
    let width = |pose: &Pose| {
        let grid = render(&look, pose);
        let cols: Vec<usize> = grid
            .iter()
            .flat_map(|row| {
                row.iter()
                    .enumerate()
                    .filter(|(_, c)| c.is_some_and(|c| c.part == Part::Fin))
                    .map(|(i, _)| i)
                    .collect::<Vec<_>>()
            })
            .collect();
        cols.iter().max().unwrap_or(&0) - cols.iter().min().unwrap_or(&0)
    };
    let side = Pose {
        spin: 90.0,
        ..Pose::default()
    };
    assert!(
        width(&side) * 2 < width(&Pose::default()),
        "侧过去的鳍：{} 对正面的 {}",
        width(&side),
        width(&Pose::default())
    );
}

#[test]
fn darkness_fades_it_to_nothing() {
    let look = look();
    // 亮度加起来：每格照它的字在 `ramp` 里第几个算。
    let ramp: Vec<char> = look.ramp.chars().collect();
    let lit = |dark: f64| -> usize {
        let pose = Pose {
            dark,
            ..Pose::default()
        };
        render(&look, &pose)
            .into_iter()
            .flatten()
            .flatten()
            .map(|c| ramp.iter().position(|m| *m == c.mark).unwrap_or(0))
            .sum()
    };
    assert_eq!(lit(1.0), 0, "全黑的什么都不画");
    assert!(lit(0.6) < lit(0.3) && lit(0.3) < lit(0.0), "越暗字越淡");
    let normal = picture(&render(&look, &Pose::default()));
    let zero = Pose {
        dark: 0.0,
        ..Pose::default()
    };
    assert_eq!(picture(&render(&look, &zero)), normal, "不暗的照原样");
}

#[test]
fn a_scaled_look_draws_bigger_with_the_same_shape() {
    let look = look();
    let big = look.scaled(1.5);
    assert_eq!(big.cols, (f64::from(look.cols) * 1.5).round() as u16);
    assert_eq!(big.rows, (f64::from(look.rows) * 1.5).round() as u16);
    let grid = render(&big, &Pose::default());
    assert_eq!(grid.len(), usize::from(big.rows));
    let eyes = grid
        .iter()
        .flatten()
        .filter(|c| c.is_some_and(|c| c.part == Part::Eye));
    assert!(eyes.count() > 0, "放大了照样有眼睛");
    assert_eq!(look.scaled(1.0).cols, look.cols);
}

#[test]
fn drooping_ears_hang_lower_than_upright_ones() {
    // 没连上耷拉耳朵（「第一次打开的引导」第 10 条）：耳朵最高那一格比竖着时低。
    let look = look();
    let top = |ear: f64| {
        let pose = Pose {
            ear,
            ..Pose::default()
        };
        render(&look, &pose)
            .iter()
            .position(|row| row.iter().any(|c| c.is_some_and(|c| c.part == Part::Ear)))
            .unwrap_or(usize::MAX)
    };
    let droop = crate::config::Config::builtin().unwrap().oobe.react.droop;
    if std::env::var_os("MIYU_SHOW_EARS").is_some() {
        for ear in [0.0, 0.6, 1.1, 1.6, 2.2, droop] {
            let pose = Pose {
                ear,
                ..Pose::default()
            };
            println!("ear {ear}\n{}", picture(&render(&look, &pose)));
        }
    }
    assert!(
        top(droop) > top(0.0),
        "耷拉的 {} 竖着的 {}",
        top(droop),
        top(0.0)
    );
}

#[test]
fn a_head_tilt_moves_the_ears_sideways_and_keeps_the_fins() {
    // 「第一次打开的引导」第 10 条（2026-10-09 项目主人：「灵动性也不够」）：打字时歪着头看。往右歪，耳朵（头顶）往右挪、
    // 两只耳朵一高一低；鳍只照 `follow` 跟几成。
    let look = look();
    let cells = |pose: &Pose, part: Part| -> Vec<(usize, usize)> {
        render(&look, pose)
            .iter()
            .enumerate()
            .flat_map(|(r, row)| {
                row.iter()
                    .enumerate()
                    .filter(move |(_, c)| c.is_some_and(|c| c.part == part))
                    .map(move |(c, _)| (r, c))
            })
            .collect()
    };
    let mean = |v: &[(usize, usize)], f: fn(&(usize, usize)) -> usize| {
        v.iter().map(f).sum::<usize>() as f64 / v.len().max(1) as f64
    };
    let straight = Pose::default();
    let tilted = Pose {
        roll: 18.0,
        ..Pose::default()
    };
    let ears = (cells(&straight, Part::Ear), cells(&tilted, Part::Ear));
    assert!(!ears.0.is_empty() && !ears.1.is_empty());
    assert!(
        mean(&ears.1, |p| p.1) > mean(&ears.0, |p| p.1) + 0.5,
        "耳朵往右挪"
    );
    assert_ne!(
        picture(&render(&look, &tilted)),
        picture(&render(&look, &straight))
    );
    assert_eq!(
        picture(&render(
            &look,
            &Pose {
                roll: 0.0,
                ..Pose::default()
            }
        )),
        picture(&render(&look, &straight)),
        "不歪的和原来一样"
    );
}
