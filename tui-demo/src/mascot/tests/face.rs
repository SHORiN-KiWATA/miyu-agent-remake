//! 嘴和肚子上的圈（蓝图 `tui.md`「空会话的首页」第 5、9 条）：锯齿线是嘴、张开时中间挖空；肚子上是一圈线、圈里是身子；
//! 被列表顶上去、走下来时嘴怎么张。

use std::time::{Duration, Instant};

use super::super::{Part, Perch, Pose, render};
use super::look;

#[test]
fn the_belly_is_a_ring_of_line_with_body_inside() {
    // 2026-09-30 项目主人：那个圈不是嘴，是肚子上的纹路：一圈线，里面是身子，不挖空（同一天做过灯，又定不要）。
    let look = look();
    let b = &look.face.belly;
    let row = (look.center_row - b.center[1] * look.radius / look.cell_aspect) as usize;
    let col = usize::from(look.cols / 2);
    let grid = render(&look, &Pose::facing(0.0, 0.0));
    let inside = grid[row][col].expect("圈里不挖空");
    assert_eq!(inside.part, Part::Head, "圈里是身子");
    assert_ne!(inside.mark, look.face.line_mark);
    let ring = grid[row]
        .iter()
        .filter(|c| c.is_some_and(|c| c.mark == look.face.line_mark))
        .count();
    assert!(ring >= 2, "圈两边各一道线：{ring}");
}

#[test]
fn the_zigzag_is_the_mouth_and_opens_in_the_middle() {
    // 同一天：锯齿线是嘴，平常合着就是一道线；张嘴时上下两排分开，中间是嘴里（挖空）。
    let look = look();
    let col = usize::from(look.cols / 2);
    let peak = look.face.line[3];
    let row = (look.center_row - peak[1] * look.radius / look.cell_aspect) as usize;
    let shut = render(&look, &Pose::facing(0.0, 0.0));
    assert_eq!(
        shut[row][col].map(|c| c.mark),
        Some(look.face.line_mark),
        "合着是一道线"
    );
    let open = render(
        &look,
        &Pose {
            mouth: 1.0,
            ..Pose::facing(0.0, 0.0)
        },
    );
    assert!(open[row][col].is_none(), "张开：中间是嘴里");
    let holes =
        |g: &[Vec<Option<super::render::Cell>>]| g.iter().flatten().filter(|c| c.is_none()).count();
    assert!(holes(&open) > holes(&shut), "张嘴多出一块洞");
}

#[test]
fn being_pushed_up_is_a_hop_and_walking_down_keeps_the_mouth_open() {
    // 2026-09-30：网页里跳起、往下掉时张嘴；终端界面照被列表顶上去（张一下）、走下来（一路张着）对上。
    let perch = look().perch;
    let mut p = Perch::default();
    let t0 = Instant::now();
    assert_eq!(p.place(10, None, t0, &perch), Some(10));
    assert!(!p.take_hop(), "没动");
    assert_eq!(p.place(10, Some(4), t0, &perch), Some(4));
    assert!(p.take_hop(), "被顶上去：跳了一下");
    assert!(!p.take_hop(), "只交回一次");
    assert_eq!(p.place(10, Some(4), t0, &perch), Some(4));
    assert!(!p.take_hop(), "停在上面不算");
    let closed = t0 + Duration::from_millis(100);
    p.place(10, None, closed, &perch);
    assert!(!p.walking(closed, &perch), "关了先停一会儿");
    let walk = closed + Duration::from_millis(perch.settle_ms + perch.row_ms);
    p.place(10, None, walk, &perch);
    assert!(p.walking(walk, &perch), "一行一行走下来");
    let home = walk + Duration::from_millis(perch.row_ms * 20);
    assert_eq!(p.place(10, None, home, &perch), Some(10));
    assert!(!p.walking(home, &perch), "走到了");
}
