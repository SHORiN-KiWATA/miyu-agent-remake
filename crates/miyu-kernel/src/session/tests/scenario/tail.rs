//! 场景：压缩留尾巴（`docs/blueprint/compaction.md` 第三条第 2 条，施工 6-2 下）。从最新的一组往回，一组一组地留，
//! 不超过 min(尾巴的上限, 压缩线的四分之一)；最新的一组就超的不留；这一轮要回应的话照留；一组不拆。
//!
//! 替身的组装一条事件一行，尾巴按事件的内容估（字节除以 4）：话说得长短由剧本定。压缩线 = 窗口 − 20；锚报 20000，
//! 一定过线。

use super::*;
use crate::session::Compaction;

/// 一个会压缩的替身，尾巴的上限是 `tail`。
fn with_tail(tail: u64) -> Stage {
    let make = move || {
        let mut policy = policy();
        policy.compaction = Some(Compaction {
            reserve_cap: 10,
            margin: 10,
            tail,
            lead: 0,
            price: crate::estimate::Flat {
                image: 50,
                file: 50,
            },
            rebuild: None,
            pause: None,
            shorten: None,
            isolate: false,
        });
        policy
    };
    Stage::new(make, environment("~/src/miyu"), at(0))
}

/// `tokens` 个 token 的一段话：四个字节一个。
fn words(tokens: usize) -> String {
    "abcd".repeat(tokens)
}

/// 两轮：第一轮说 50、回 50；第二轮说 10、回 10，报 20000 当锚。再交压缩线 `line`，第三轮说 10。
/// 日志：2 第一句、3 开回合、4、5 两块事实、6 回复、7、8；9 第二句、10、11 回复、12、13；14 第三句、15。
fn three_turns(stage: &mut Stage, line: u64) {
    stage.model([
        Line::says(&words(50)),
        Line::says(&words(10)).reports(20_000),
        Line::says("S1"),
        Line::says("好。"),
    ]);
    stage.say(&words(50));
    stage.say(&words(10));
    stage.limits(Some(line + 20), None);
    stage.say(&words(10));
}

/// 第 `k` 次请求看到了第几条为止。
fn seen(stage: &Stage, k: usize) -> u64 {
    stage.requests()[k].0.get()
}

#[test]
fn groups_are_kept_from_the_newest_back_until_the_budget() {
    // 预算 100：第三句 10、第二轮的回复 10、第二句 10、第一轮的回复 50，一共 80；再加第一句那一组（50 加两块事实）
    // 就超了。尾巴从第一轮的回复起，替代到 5。
    let mut stage = with_tail(100);
    three_turns(&mut stage, 10_000);
    assert_eq!(seen(&stage, 2), 5);
    let (_, main) = &stage.requests()[3];
    assert!(
        listed_request(main)
            .starts_with("6 message.assistant\n7 model.called\n8 turn.ended\n9 message.user\n"),
        "尾巴原样排在检查点后面：{}",
        listed_request(main)
    );
}

#[test]
fn the_budget_is_at_most_a_quarter_of_the_line() {
    // 上限 1000，线 200：预算 50。第三句、第二轮的回复、第二句一共 30，再加第一轮的回复 50 就超了。
    let mut stage = with_tail(1000);
    three_turns(&mut stage, 200);
    assert_eq!(seen(&stage, 2), 8);
}

#[test]
fn a_zero_budget_keeps_only_what_this_turn_answers() {
    let mut stage = with_tail(0);
    three_turns(&mut stage, 10_000);
    assert_eq!(seen(&stage, 2), 13, "只留第三句");
}

#[test]
fn a_newest_group_over_the_budget_leaves_no_tail() {
    // 回合中途，线 400（预算 100）：最新的一组是调工具的回复和它 1000 个 token 的结果，超了预算，不留尾巴，替代到
    // 最后一条。
    let mut stage = with_tail(100);
    stage.limits(Some(420), None);
    stage.model([
        Line::calls("", &[("read", "{}")]).reports(1000),
        Line::says("S1"),
        Line::says("看完了。").reports(10),
    ]);
    stage.tools([Play::Done(words(1000))]);
    stage.say("看看");
    assert_eq!(seen(&stage, 1), 8);
    // 结果只有 10 个 token 的，回复和它的结果都留下，连同这一轮的那句和两块事实：替代到 1。前面没有能压的，不压。
    let mut stage = with_tail(100);
    stage.limits(Some(420), None);
    stage.model([
        Line::calls("", &[("read", "{}")]).reports(1000),
        Line::says("看完了。").reports(10),
    ]);
    stage.tools([Play::Done(words(10))]);
    stage.say("看看");
    assert_eq!(stage.requests().len(), 2, "一次都没压");
}

#[test]
fn what_this_turn_answers_stays_even_over_the_budget() {
    // 第三句 1000 个 token，比预算大：照样留着，替代到它前面。
    let mut stage = with_tail(100);
    stage.model([
        Line::says(&words(50)),
        Line::says(&words(10)).reports(20_000),
        Line::says("S1"),
        Line::says("好。"),
    ]);
    stage.say(&words(50));
    stage.say(&words(10));
    stage.limits(Some(10_020), None);
    stage.say(&words(1000));
    assert_eq!(seen(&stage, 2), 13);
}

/// 重试次数和这一步的主请求合用一个计数（施工 6-2 下）：主请求出错一次、等着再来时线收紧，到点先压，摘要说完了不清零；
/// 之后主请求再错五次，这一步一共六次，这一轮出错结束。
#[test]
fn a_summary_in_the_middle_of_retries_does_not_reset_the_count() {
    let mut stage = with_tail(0);
    stage.model([
        Line::says(&words(50)),
        Line::says(&words(10)).reports(20_000),
    ]);
    stage.say(&words(50));
    stage.say(&words(10));
    stage.limits(Some(100_000), None);
    stage.hold_wakes();
    let fails = || Line::fails(ErrorClass::Retryable, "503");
    let mut lines = vec![fails(), Line::says("S1")];
    lines.extend((0..5).map(|_| fails()));
    stage.model(lines);
    stage.say(&words(10));
    stage.limits(Some(10_020), None);
    stage.unhold_wakes();
    stage.release_wake();
    assert_eq!(stage.requests().len(), 2 + 1 + 1 + 5);
    let last = stage.log().last().map(told);
    assert!(
        last.as_deref()
            .is_some_and(|line| line.ends_with("turn.ended:error kernel t15")),
        "{:#?}",
        story(&stage)
    );
}

/// 摘要请求的个数。
fn summaries(stage: &Stage) -> usize {
    stage
        .requests()
        .iter()
        .filter(|(_, request)| listed_request(request).ends_with("summarize\n"))
        .count()
}

/// 一步至多压一次（施工 6-2 下，`compaction.md` 第三条第 1 条）：压完再注入的两块事实并进这一轮的那一组，最新的一组
/// 就比预算大了，再算一次能替代到更后面；可是这一步压过了，照发主请求。
#[test]
fn a_step_is_compacted_at_most_once() {
    let mut stage = with_tail(30);
    stage.model([
        Line::says("abcd").reports(20_000),
        Line::says("S1"),
        Line::says("S2"),
        Line::says("好。"),
    ]);
    stage.say("abcd");
    stage.limits(Some(40), None);
    stage.say("abcd");
    assert_eq!(summaries(&stage), 1, "{:#?}", story(&stage));
}

/// 发了主请求，下一步又能压：回合开头压过，回合中途再过线，照样再压。
#[test]
fn the_next_step_can_be_compacted_again() {
    let mut stage = with_tail(0);
    stage.model([
        Line::says(&words(10)).reports(20_000),
        Line::says("S1"),
        Line::calls("", &[("read", "{}")]).reports(20_000),
        Line::says("S2"),
        Line::says("好。"),
    ]);
    stage.tools([Play::Done("lib.rs".to_string())]);
    stage.say(&words(10));
    stage.limits(Some(120), None);
    stage.say(&words(10));
    assert_eq!(summaries(&stage), 2, "{:#?}", story(&stage));
}

/// 图片照限额里驱动交的算法估（施工 6-3 上）：一张图的那一组，照固定的 50 算就超了预算，照驱动的 1 算就留在尾巴里。
#[test]
fn the_tail_counts_images_with_the_driver_price() {
    struct One;
    impl crate::estimate::ImagePrice for One {
        fn tokens(&self, _: u32, _: u32) -> u64 {
            1
        }
    }
    let image = || {
        vec![Block::Image(crate::block::Image {
            blob: crate::id::ContentHash::of(b"png"),
            name: None,
            media_type: crate::id::MediaType::parse("image/png").unwrap(),
            width: 1000,
            height: 500,
        })]
    };
    // 第一轮发一张图、答 5；第二轮说 10、答 10 报 20000；第三轮说 10 就压，预算 100。从新往旧：第三句 10、第二轮的
    // 回复 10、第二句 10、第一轮的回复 5，一共 35；再加发图那一组（图加两块事实约 20）：图照固定的 50 就超了，尾巴从
    // 第一轮的回复起，替代到 5；照驱动的 1 超不过，尾巴一直留到开头，前面没有能压的，不压。
    let run = |images: Option<std::sync::Arc<dyn crate::estimate::ImagePrice>>| {
        let mut stage = with_tail(100);
        stage.model([
            Line::says(&words(5)),
            Line::says(&words(10)).reports(20_000),
            Line::says("S1"),
            Line::says("好。"),
        ]);
        stage.send(image());
        stage.say(&words(10));
        stage.limits_with(crate::session::Limits {
            model: crate::testkit::model(),
            window: Some(10_020),
            max_output: None,
            images,
            blind: false,
        });
        stage.say(&words(10));
        summaries(&stage)
    };
    assert_eq!(run(None), 1);
    assert_eq!(run(Some(std::sync::Arc::new(One))), 0);
}
