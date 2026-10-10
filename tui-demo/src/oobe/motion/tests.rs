//! 动的东西一帧帧查（「第一次打开的引导」第 8–11、29 条）。

use std::time::{Duration, Instant};

use super::{Intro, React, Slide, Stars};
use crate::config::Config;
use crate::oobe::look::Look;

fn look() -> Look {
    Config::builtin().unwrap().oobe
}

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

#[test]
fn the_intro_spins_in_from_black_then_types_and_ends_ready() {
    let look = look().intro;
    let t0 = Instant::now();
    let intro = Intro::new(t0);
    let first = intro.scene(t0, &look, 8);
    assert_eq!(first.dark, 1.0, "一开始全黑");
    assert!(first.spin <= -359.0, "从背后转起：{}", first.spin);
    assert_eq!(first.typed, 0);
    assert!(!first.ready);
    // 转到一半：亮了一些、还没转到正面。
    let mid = intro.scene(t0 + ms(look.spin_ms / 2), &look, 8);
    assert!(mid.dark < 1.0 && mid.dark > 0.0, "{}", mid.dark);
    assert!(mid.spin < 0.0 && mid.spin > -360.0);
    // 转完、亮了，还没开始打字；这中间抖耳朵、眨眼。
    let landed = t0 + ms(look.spin_ms.max(look.fade_ms));
    let settle: Vec<_> = (0..look.settle_ms)
        .step_by(10)
        .map(|d| intro.scene(landed + ms(d), &look, 8))
        .collect();
    assert!(
        settle
            .iter()
            .all(|s| s.spin == 0.0 && s.dark == 0.0 && s.typed == 0)
    );
    assert!(settle.iter().any(|s| s.ear > 0.0), "落定以后抖一下耳朵");
    let blinks = settle
        .windows(2)
        .filter(|w| !w[0].blink && w[1].blink)
        .count();
    assert_eq!(blinks, 2, "连眨两下");
    // 打字：一个字一个字，嘴一张一合。
    let typing = landed + ms(look.settle_ms);
    let words: Vec<_> = (0..8)
        .map(|i| intro.scene(typing + ms(look.type_ms * i + look.type_ms / 2), &look, 8))
        .collect();
    let typed: Vec<usize> = words.iter().map(|s| s.typed).collect();
    assert_eq!(typed, (1..=8).collect::<Vec<_>>(), "一个字一个字");
    assert!(words.iter().any(|s| s.mouth > 0.0) && words.iter().any(|s| s.mouth == 0.0));
    // 打完：说明淡出来，最后露出「回车开始」，嘴合上。
    let typed_end = typing + ms(look.type_ms * 8);
    let fading = intro.scene(typed_end + ms(look.sub_ms / 2), &look, 8);
    assert!(fading.sub > 0.0 && fading.sub < 1.0 && !fading.ready);
    let end = intro.scene(typed_end + ms(look.sub_ms), &look, 8);
    assert!(end.ready && end.sub == 1.0 && end.mouth == 0.0 && end.typed == 8);
    assert!(intro.finished(typed_end + ms(look.sub_ms), &look, 8));
}

#[test]
fn any_key_skips_to_the_last_frame() {
    let look = look().intro;
    let t0 = Instant::now();
    let mut intro = Intro::new(t0);
    intro.skip();
    let scene = intro.scene(t0, &look, 5);
    assert!(scene.ready && scene.typed == 5 && scene.dark == 0.0 && scene.spin == 0.0);
    assert!(!scene.blink && scene.mouth == 0.0 && scene.ear == 0.0);
}

#[test]
fn stars_gather_to_the_middle_then_a_few_twinkle_and_they_scatter_away() {
    let look = look().stars;
    let t0 = Instant::now();
    let center = (0.5, 0.4);
    let mut stars = Stars::gather(t0, 7);
    let first = stars.at(t0, &look, center);
    assert_eq!(first.len(), look.count, "开场冒出这么多颗");
    let far = |list: &[super::Star]| {
        list.iter()
            .map(|s| ((s.x - center.0).powi(2) + (s.y - center.1).powi(2)).sqrt())
            .sum::<f64>()
            / list.len().max(1) as f64
    };
    let later = stars.at(t0 + ms(look.gather_ms * 3 / 4), &look, center);
    assert!(
        far(&later) < far(&first),
        "往中间聚：{} → {}",
        far(&first),
        far(&later)
    );
    assert!(
        first
            .iter()
            .chain(&later)
            .all(|s| (0.0..=1.0).contains(&s.x) && (0.0..=1.0).contains(&s.y)),
        "都在窗口里"
    );
    assert!(stars.moving(t0 + ms(10), &look));
    // 聚完：只剩四周闪着的几颗，离中间不近，一闪一闪（亮暗会变）。
    let after = t0 + ms(look.gather_ms + 50);
    let calm = stars.at(after, &look, center);
    assert_eq!(calm.len(), look.linger);
    assert!(!stars.moving(after, &look), "只剩闪的不按动着的节拍画");
    let levels: Vec<f64> = (0..20)
        .map(|i| stars.at(after + ms(i * 150), &look, center)[0].level)
        .collect();
    assert!(
        levels.iter().any(|l| (l - levels[0]).abs() > 0.1),
        "会闪：{levels:?}"
    );
    // 同一个种子一样的星，换种子不一样。
    assert_eq!(Stars::gather(t0, 7).at(t0, &look, center), first);
    assert_ne!(Stars::gather(t0, 8).at(t0, &look, center), first);
    // 散开：越走越远、越来越暗，走完就没了。
    let gone = after + ms(1000);
    stars.scatter(gone);
    let out = stars.at(gone + ms(look.scatter_ms / 2), &look, center);
    assert!(!out.is_empty() && stars.moving(gone + ms(10), &look));
    let edge = |s: &&super::Star| s.x == 0.0 || s.x == 1.0 || s.y == 0.0 || s.y == 1.0;
    assert!(
        out.iter().filter(edge).count() <= 1,
        "飞出去的不贴着边堆成一条线：{out:?}"
    );
    assert!(
        stars
            .at(gone + ms(look.scatter_ms + 10), &look, center)
            .is_empty()
    );
    assert!(
        Stars::quiet(t0, 7).at(t0, &look, center).len() == look.linger,
        "回到欢迎页只有闪的"
    );
}

#[test]
fn a_step_slides_out_left_and_the_next_comes_in_from_the_right() {
    let total = ms(look().slide_ms);
    let t0 = Instant::now();
    let slide = Slide::new(t0, false);
    assert_eq!(slide.offsets(t0, total, 60), Some((0, 60)));
    let (old, new) = slide.offsets(t0 + total / 2, total, 60).expect("还在滑");
    assert!(old < 0 && new > 0 && new < 60, "{old} {new}");
    assert!(-old > 30, "先快后慢：一半时已经过半");
    assert_eq!(slide.offsets(t0 + total, total, 60), None, "滑完了");
    let back = Slide::new(t0, true);
    let (old, new) = back.offsets(t0 + total / 2, total, 60).unwrap();
    assert!(old > 0 && new < 0, "回上一步反过来");
}

#[test]
fn the_mascot_hops_droops_shakes_and_cheers() {
    let look = look().react;
    let t0 = Instant::now();
    let mut react = React::default();
    assert_eq!(react.act(t0, &look), super::react::Act::default());
    react.hop(t0);
    let up = react.act(t0 + ms(look.hop_ms / 2), &look);
    assert_eq!(up.lift, look.hop_rows, "跳起来");
    assert!(up.mouth > 0.0, "张一下嘴");
    assert_eq!(react.act(t0 + ms(look.hop_ms), &look).lift, 0, "落下来");
    assert!(react.busy(t0, &look) && !react.busy(t0 + ms(look.hop_ms + 1), &look));
    // 等着：抬头。
    react.waiting = true;
    assert_eq!(react.act(t0, &look).pitch, Some(look.look_up));
    react.waiting = false;
    // 没连上：耷拉耳朵、摇头，摇完耳朵还耷拉着，按键竖回来。
    react.sad(t0);
    let yaws: Vec<f64> = (0..look.shake_ms)
        .step_by(20)
        .map(|d| react.act(t0 + ms(d), &look).yaw)
        .collect();
    assert!(
        yaws.iter().any(|y| *y > 1.0) && yaws.iter().any(|y| *y < -1.0),
        "左右摇"
    );
    let after = react.act(t0 + ms(look.shake_ms + 10), &look);
    assert_eq!(after.yaw, 0.0);
    assert_eq!(after.ear, look.droop, "耳朵还耷拉着");
    react.poke();
    assert_eq!(react.act(t0 + ms(look.shake_ms + 10), &look).ear, 0.0);
    // 连上了：跳一下、连眨两下。
    react.sad(t0);
    react.happy(t0);
    let frames: Vec<_> = (0..(look.blink_ms * 2 + look.blink_gap_ms + 50))
        .step_by(10)
        .map(|d| react.act(t0 + ms(d), &look))
        .collect();
    assert_eq!(
        frames
            .windows(2)
            .filter(|w| !w[0].blink && w[1].blink)
            .count()
            + usize::from(frames[0].blink),
        2
    );
    assert!(frames.iter().any(|a| a.lift > 0));
    assert_eq!(frames[0].ear, 0.0, "高兴时耳朵竖着");
}

#[test]
fn the_spin_winds_up_first_and_overshoots_before_settling() {
    // 2026-10-09 项目主人：「吉祥物旋转的动画可以有一个曲线，动画时间比现在要久一些，不然不够有视觉冲击力」。
    let look = look().intro;
    assert!(look.spin_ms >= 2000, "比原来的 1.5 秒久");
    let t0 = Instant::now();
    let intro = Intro::new(t0);
    let at = |share: u64| {
        intro
            .scene(t0 + ms(look.spin_ms * share / 100), &look, 8)
            .spin
    };
    assert!(at(10) < -360.0, "先往反方向蓄力：{}", at(10));
    assert!(at(90) > 0.0, "冲过正面一点：{}", at(90));
    assert!(
        at(50) > -200.0 && at(50) < -160.0,
        "中间转得最快、走到一半：{}",
        at(50)
    );
    assert_eq!(at(100), 0.0, "最后回正");
}

#[test]
fn typing_tilts_the_head_until_a_pause_and_a_new_step_gets_a_hop() {
    // 「第一次打开的引导」第 10 条（2026-10-09 项目主人：「灵动性也不够，略显死板」）。
    let look = look().react;
    let t0 = Instant::now();
    let mut react = React::default();
    react.typed(t0, &look);
    react.typed(t0 + ms(200), &look);
    let typing = react.act(t0 + ms(look.tilt_ease_ms + 100), &look);
    assert!(typing.roll > look.tilt * 0.9, "歪过去了：{}", typing.roll);
    assert!(react.busy(t0 + ms(look.tilt_ease_ms + 100), &look));
    let after = t0 + ms(200 + look.tilt_hold_ms + look.tilt_ease_ms + 10);
    assert_eq!(react.act(after, &look).roll, 0.0, "停手一阵回正");
    assert!(!react.busy(after, &look), "回正了不再画");
    let hop_look = look.clone();
    react.hop(after);
    let hop = react.act(after + ms(hop_look.hop_ms / 2), &hop_look);
    assert!(
        hop.lift >= 2 && hop.ear > 0.0,
        "跳两行、耳朵扑一下：{hop:?}"
    );
}
