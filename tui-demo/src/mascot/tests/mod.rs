//! 吉祥物（蓝图 `tui.md`「空会话的首页」第 5–10 条）：画出来的大小、正面对称、五官跟着转、嘴和肚子上的圈、转头的角度和缓动。

use std::time::{Duration, Instant};

use super::{Gaze, Idle, Part, Perch, Pose, reaching, render, toward};
use crate::config::Config;

fn look() -> super::Look {
    Config::builtin().unwrap().mascot
}

/// 眼睛那几格的平均位置：(列, 行)。
fn eyes(grid: &[Vec<Option<super::render::Cell>>]) -> (f64, f64, usize) {
    let mut sum = (0.0, 0.0, 0);
    for (r, row) in grid.iter().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            if cell.is_some_and(|x| x.part == Part::Eye) {
                sum = (sum.0 + c as f64, sum.1 + r as f64, sum.2 + 1);
            }
        }
    }
    (
        sum.0 / sum.2.max(1) as f64,
        sum.1 / sum.2.max(1) as f64,
        sum.2,
    )
}

#[test]
fn the_picture_has_the_configured_size_and_a_symmetric_front() {
    let mut look = look();
    // 光正对着脸：明暗也左右对称，整张画左右镜像。
    look.light = [0.0, 0.0, 1.0];
    let grid = render(&look, &Pose::facing(0.0, 0.0));
    assert_eq!(grid.len(), usize::from(look.rows));
    assert!(grid.iter().all(|r| r.len() == usize::from(look.cols)));
    let marks = |r: &Vec<Option<super::render::Cell>>| -> String {
        r.iter().map(|c| c.map_or(' ', |c| c.mark)).collect()
    };
    for row in &grid {
        let line = marks(row);
        let mirrored: String = line.chars().rev().collect();
        assert_eq!(line, mirrored, "正面左右对称");
    }
    assert!(
        grid.iter()
            .flatten()
            .any(|c| c.is_some_and(|c| c.part == Part::Ear)),
        "有耳朵"
    );
    assert!(
        grid.iter()
            .flatten()
            .any(|c| c.is_some_and(|c| c.part == Part::Fin)),
        "有鳍"
    );
}

#[test]
fn the_eyes_follow_the_turn() {
    let look = look();
    let front = eyes(&render(&look, &Pose::facing(0.0, 0.0)));
    assert!(front.2 >= 2, "正面看得见眼睛");
    let centre = f64::from(look.cols) / 2.0 - 0.5;
    assert!((front.0 - centre).abs() < 0.01, "两只眼睛左右对称");
    let left = eyes(&render(&look, &Pose::facing(-30.0, 0.0)));
    assert!(left.0 < front.0 - 1.0, "往左看，眼睛往左挪");
    let down = eyes(&render(&look, &Pose::facing(0.0, 15.0)));
    assert!(down.1 > front.1 + 0.5, "往下看，眼睛往下挪");
}

#[test]
fn the_head_turns_toward_the_target_up_to_its_limits() {
    let g = look().gaze;
    assert_eq!(
        toward((20.0, 8.0), (20.0, 8.0), 2.1, &g),
        (0.0, 0.0),
        "就在脸上：正前方"
    );
    let (yaw, pitch) = toward((20.0, 8.0), (10.0, 20.0), 2.1, &g);
    assert!(yaw < 0.0 && pitch > 0.0, "左下方：往左、往下");
    let expected = (-10.0_f64).atan2(g.distance).to_degrees();
    assert!(
        (yaw - expected).abs() < 1e-9,
        "左右照 atan(横向距离 ÷ 虚拟距离)"
    );
    let far = toward((20.0, 8.0), (-500.0, -500.0), 2.1, &g);
    assert_eq!(far, (-g.max_yaw, -g.max_pitch), "到上限就停");
}

#[test]
fn far_away_the_head_still_follows_the_target() {
    let g = look().gaze;
    // 侧边栏：脸在第 140 列，输入光标在第 10 列、第 100 列。
    let face = (140.0, 8.0);
    let far = reaching(&g, 140.0);
    let left = toward(face, (10.0, 8.0), 2.1, &far).0;
    let right = toward(face, (100.0, 8.0), 2.1, &far).0;
    assert!(
        left < right && right < 0.0,
        "都朝左，光标往右挪头跟着回来一些：{left} {right}"
    );
    let edge = toward(face, (0.0, 8.0), 2.1, &far).0;
    assert!((edge + g.max_yaw).abs() < 1e-9, "屏幕最左边刚好转到头");
    // 照首页的距离：两处都转到头，看不出来。
    assert_eq!(
        toward(face, (10.0, 8.0), 2.1, &g).0,
        toward(face, (100.0, 8.0), 2.1, &g).0
    );
    // 近处不用放远：首页照旧。
    assert_eq!(reaching(&g, 10.0).distance, g.distance);
}

#[test]
fn turning_eases_half_way_per_half_life_and_stops_at_the_target() {
    let half = Duration::from_millis(40);
    let mut gaze = Gaze::default();
    let t0 = Instant::now();
    gaze.step(t0, half);
    gaze.aim((20.0, 10.0));
    gaze.step(t0 + half, half);
    let (yaw, pitch) = gaze.angles();
    assert!(
        (yaw - 10.0).abs() < 1e-6 && (pitch - 5.0).abs() < 1e-6,
        "一个半衰期走一半"
    );
    assert!(gaze.moving());
    // 很久没画：一帧最多走一个半衰期，不一下子跳到。
    gaze.step(t0 + Duration::from_secs(5), half);
    assert!((gaze.angles().0 - 15.0).abs() < 1e-6);
    for i in 0..40 {
        gaze.step(t0 + Duration::from_secs(5) + half * (i + 1), half);
    }
    assert_eq!(gaze.angles(), (20.0, 10.0), "到了停在目标上");
    assert!(!gaze.moving());
}

/// 量尺：打出正面和几个转头的样子，调模型、明暗时看（`cargo test mascot_preview -- --ignored --nocapture`）。
/// 每一帧先是字，再是颜色记号：h 头、e 耳朵、y 眼睛、f 鳍。
#[test]
#[ignore = "量尺：看样子用"]
fn mascot_preview() {
    // 调参时指一份别的 json，不用重编。
    let look = std::env::var("MIYU_MASCOT_PREVIEW").map_or_else(
        |_| look(),
        |path| serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap(),
    );
    let front = Pose::facing(0.0, 0.0);
    let poses = [
        ("正面", front),
        (
            "嘴张开",
            Pose {
                // 看张到一半的样子：`MIYU_MASCOT_MOUTH=0.6`。
                mouth: std::env::var("MIYU_MASCOT_MOUTH")
                    .ok()
                    .and_then(|m| m.parse().ok())
                    .unwrap_or(1.0),
                ..front
            },
        ),
        ("左下", Pose::facing(-30.0, 10.0)),
        ("右下", Pose::facing(30.0, 15.0)),
        ("往上", Pose::facing(0.0, -15.0)),
    ];
    for (name, pose) in poses {
        println!("=== {name}");
        let grid = render(&look, &pose);
        for row in &grid {
            let marks: String = row.iter().map(|c| c.map_or(' ', |c| c.mark)).collect();
            println!("{}", marks.trim_end());
        }
        println!("--- mask");
        for row in &grid {
            let mask: String = row
                .iter()
                .map(|c| {
                    c.map_or('.', |c| match c.part {
                        Part::Head => 'h',
                        Part::Ear => 'e',
                        Part::Eye => 'y',
                        Part::Fin => 'f',
                    })
                })
                .collect();
            println!("{mask}");
        }
    }
}

#[test]
fn a_blink_fills_the_eyes_and_a_twitch_tips_the_ears_outwards() {
    let look = look();
    let open = render(&look, &Pose::facing(0.0, 0.0));
    let shut = render(
        &look,
        &Pose {
            blink: true,
            ..Pose::facing(0.0, 0.0)
        },
    );
    assert!(eyes(&open).2 > 0);
    assert_eq!(eyes(&shut).2, 0, "闭眼：那两道空洞填上");
    // 左边那只耳朵往左歪：它的格子平均往左挪。
    let half = usize::from(look.cols / 2);
    let left_ear = |grid: &[Vec<Option<super::render::Cell>>]| {
        let cols: Vec<f64> = grid
            .iter()
            .flat_map(|r| r.iter().enumerate().take(half))
            .filter(|(_, c)| c.is_some_and(|c| c.part == Part::Ear))
            .map(|(i, _)| i as f64)
            .collect();
        cols.iter().sum::<f64>() / cols.len().max(1) as f64
    };
    let tipped = render(
        &look,
        &Pose {
            ear: look.idle.twitch_tilt[1],
            ..Pose::facing(0.0, 0.0)
        },
    );
    assert!(left_ear(&tipped) < left_ear(&open) - 0.5, "耳朵往外抖");
}

fn ms(t0: Instant, n: u64) -> Instant {
    t0 + Duration::from_millis(n)
}

#[test]
fn it_glances_around_in_spells_only_while_nobody_moves() {
    let cfg = look().idle;
    let mut idle = Idle::new(7);
    let t0 = Instant::now();
    assert!(idle.pose(t0, &cfg).glance.is_none(), "刚打开：还没到待机");
    assert!(idle.pose(ms(t0, cfg.after_ms - 1), &cfg).glance.is_none());
    // 待机以后一阵一阵换角度：每一次都在范围里，停着的那一阵不变，前后几次幅度不一样，有时回正前方。
    let mut seen = Vec::new();
    let mut t = cfg.after_ms;
    while t < cfg.after_ms + 120_000 {
        let g = idle.pose(ms(t0, t), &cfg).glance.unwrap();
        assert!(g.0.abs() <= cfg.glance_yaw && g.1.abs() <= cfg.glance_pitch);
        if seen.last() != Some(&g) {
            seen.push(g);
        }
        t += 50;
    }
    let spells = seen.len();
    assert!(
        (120_000 / cfg.glance_hold_ms[1]) as usize <= spells,
        "停够一阵就换"
    );
    assert!(
        spells <= (120_000 / cfg.glance_hold_ms[0]) as usize + 1,
        "不是一直在动"
    );
    let mut yaws: Vec<i64> = seen.iter().map(|g| (g.0 * 10.0) as i64).collect();
    yaws.sort_unstable();
    yaws.dedup();
    assert!(yaws.len() >= spells / 2, "幅度每次不一样");
    assert!(seen.contains(&(0.0, 0.0)), "有时回正前方");
    let now = ms(t0, t);
    idle.poke(now);
    assert!(idle.pose(now, &cfg).glance.is_none(), "一动就停");
}

#[test]
fn it_does_not_redraw_while_holding_still() {
    let cfg = look().idle;
    let mut idle = Idle::new(7);
    let t0 = Instant::now();
    idle.pose(t0, &cfg);
    let now = ms(t0, cfg.after_ms + 10);
    idle.pose(now, &cfg);
    // 停着的时候下一次该画的是下一个小动作的时刻，不是一小段一帧。
    let wake = idle.wake(now, &cfg).unwrap();
    assert!(wake > now + Duration::from_millis(cfg.frame_ms), "不空转");
}

#[test]
fn blinks_vary_and_sometimes_come_in_pairs() {
    let mut cfg = look().idle;
    cfg.double_blink = 1.0;
    let mut idle = Idle::new(7);
    let t0 = Instant::now();
    // 一毫秒一步走 30 秒，记下每一次闭眼的起止。
    let mut blinks: Vec<(u64, u64)> = Vec::new();
    let mut shut: Option<u64> = None;
    for t in 0..30_000 {
        match (idle.pose(ms(t0, t), &cfg).blink, shut) {
            (true, None) => shut = Some(t),
            (false, Some(start)) => {
                blinks.push((start, t));
                shut = None;
            }
            _ => {}
        }
    }
    assert!(blinks.len() >= 4);
    for (start, end) in &blinks {
        let long = end - start;
        assert!(
            (cfg.blink_ms[0]..=cfg.blink_ms[1] + 1).contains(&long),
            "闭眼多久在范围里"
        );
    }
    let lengths: std::collections::BTreeSet<u64> = blinks.iter().map(|(s, e)| e - s).collect();
    assert!(lengths.len() > 1, "每次闭眼长短不一样");
    let paired = blinks
        .windows(2)
        .any(|w| w[1].0 - w[0].1 <= cfg.double_gap_ms + 1);
    assert!(paired, "连眨两下");
}

#[test]
fn it_blinks_on_schedule_and_twitches_while_idle() {
    let mut cfg = look().idle;
    cfg.blink_every_ms = [3000, 3000];
    cfg.blink_ms = [150, 150];
    cfg.double_blink = 0.0;
    cfg.twitch_every_ms = [1000, 1000];
    let mut idle = Idle::new(7);
    let t0 = Instant::now();
    assert!(!idle.pose(t0, &cfg).blink);
    assert!(idle.pose(ms(t0, 3050), &cfg).blink, "3 秒一眨");
    assert!(!idle.pose(ms(t0, 3300), &cfg).blink, "闭一下就睁开");
    // 待机以后一秒一抖：抖的那一小段里耳朵往外歪，幅度在范围里。
    let lifted = (0..400)
        .map(|i| idle.pose(ms(t0, 3400 + i * 10), &cfg).ear)
        .fold(0.0_f64, f64::max);
    assert!(lifted > 0.1 && lifted <= cfg.twitch_tilt[1], "抖过耳朵");
}

#[test]
fn a_list_pushes_the_mascot_up_where_it_stays_until_it_walks_back_down() {
    let perch = look().perch;
    let mut p = Perch::default();
    let t0 = Instant::now();
    // 平常在第 10 行。
    assert_eq!(p.place(10, None, t0, &perch), Some(10));
    // 开列表：顶到第 4 行；列表变矮（只要顶到第 6 行）也不往下掉；变高再往上顶。
    assert_eq!(p.place(10, Some(4), ms(t0, 10), &perch), Some(4));
    assert_eq!(
        p.place(10, Some(6), ms(t0, 20), &perch),
        Some(4),
        "变矮不往下掉"
    );
    assert_eq!(
        p.place(10, Some(2), ms(t0, 30), &perch),
        Some(2),
        "变高再往上顶"
    );
    assert_eq!(
        p.place(10, Some(-3), ms(t0, 40), &perch),
        None,
        "上面放不下：不画"
    );
    assert_eq!(
        p.place(10, Some(2), ms(t0, 50), &perch),
        None,
        "放不下以后列表开着就一直不画"
    );
    // 关了：先停一会儿，再一行一行走下来。
    let closed = ms(t0, 100);
    assert_eq!(p.place(10, None, closed, &perch), None);
    assert!(p.wake(closed, &perch).is_some(), "还在往回走");
    let after = |n: u64| ms(closed, perch.settle_ms + perch.row_ms * n);
    let first = p.place(10, None, ms(closed, perch.settle_ms - 1), &perch);
    assert_eq!(first, None, "关了先停一会儿（放不下时还不露面）");
    let mut last = -1;
    for n in 1..=12 {
        if let Some(y) = p.place(10, None, after(n), &perch) {
            assert!(
                i32::from(y) >= last && i32::from(y) - last <= 1 || last < 0,
                "一行一行走"
            );
            last = i32::from(y);
        }
    }
    assert_eq!(last, 10, "走回原处");
    assert!(p.wake(after(12), &perch).is_none(), "到了就停");
}

#[test]
fn a_perched_mascot_waits_then_walks_down_row_by_row() {
    let perch = look().perch;
    let mut p = Perch::default();
    let t0 = Instant::now();
    p.place(10, Some(6), t0, &perch);
    let closed = ms(t0, 10);
    assert_eq!(p.place(10, None, closed, &perch), Some(6), "刚关：还停着");
    assert_eq!(
        p.place(10, None, ms(closed, perch.settle_ms - 1), &perch),
        Some(6)
    );
    assert_eq!(
        p.place(10, None, ms(closed, perch.settle_ms + perch.row_ms), &perch),
        Some(7),
        "走一行"
    );
    assert_eq!(
        p.place(
            10,
            None,
            ms(closed, perch.settle_ms + perch.row_ms * 2),
            &perch
        ),
        Some(8)
    );
    // 走到一半又开列表：就地顶上去。
    assert_eq!(
        p.place(
            10,
            Some(5),
            ms(closed, perch.settle_ms + perch.row_ms * 3),
            &perch
        ),
        Some(5)
    );
}

mod face;
