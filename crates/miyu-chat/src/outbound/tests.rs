//! 出站链（`chat.md` 第五条「守着它的」）：先后；清理（两种工具调用、没有收尾的、几段、去完以后是空的、零宽字符、括号
//! 旁白）；去重（一字不差、只差标点大小写、相似度正好 0.66、两字组 15 个的不比、只看交进来的这一回合、正文重复带图、
//! 图重复、同一条里两张一样的图）；引用和 @（最后一条是她自己的、`quote_after` 是 0、正好 4 条、3 条、@ 的时间和人）。

use super::test_support::{
    MENTION_AFTER, ctx, distinct, drop, judge, send, sent, text, with_images,
};
use super::{Out, OutChain, OutWhy, Target};

#[test]
fn builtin_has_three_rules_in_order() {
    let names: Vec<_> = OutChain::builtin()
        .rules
        .iter()
        .map(|rule| rule.name().to_string())
        .collect();
    assert_eq!(names, ["clean", "dedupe", "target"]);
}

#[test]
fn plain_text_passes_untouched() {
    assert_eq!(
        judge(text("好的，马上来"), &ctx()),
        send(text("好的，马上来"))
    );
}

#[test]
fn clean_runs_before_dedupe() {
    // 漏了工具调用的、又和发过的一样：清理先丢。旁白也一样。
    let ctx = sent(&["<tool_call>x</tool_call>", "（笑）"]);
    assert_eq!(
        judge(text("<tool_call>x</tool_call>"), &ctx),
        drop(OutWhy::Leaked)
    );
    assert_eq!(judge(text("（笑）"), &ctx), drop(OutWhy::Aside));
}

#[test]
fn both_tool_call_forms_are_stripped() {
    let tool = judge(text("好<tool_call>{\"name\":\"x\"}</tool_call>的"), &ctx());
    assert_eq!(tool, send(text("好的")));
    let function = judge(text("好<function=search>{\"q\":1}</function>的"), &ctx());
    assert_eq!(function, send(text("好的")));
}

#[test]
fn unclosed_tool_call_is_stripped_to_the_end() {
    assert_eq!(
        judge(text("好的<tool_call>{\"na"), &ctx()),
        send(text("好的"))
    );
    assert_eq!(judge(text("好的<function=x>"), &ctx()), send(text("好的")));
    // 另一种的收尾不算收尾。
    assert_eq!(
        judge(text("好<tool_call>a</function>b"), &ctx()),
        send(text("好"))
    );
}

#[test]
fn several_spans_are_all_stripped() {
    let leaky =
        "a<tool_call>1</tool_call>b<function=f>2</function>c<tool_call>3</tool_call>d<tool_call>4";
    assert_eq!(judge(text(leaky), &ctx()), send(text("abcd")));
    // 先去最早开头的一段：`<function=` 在前，它的收尾把里面的 `<tool_call>` 一起带走。
    let nested = "a<function=f><tool_call>1</function>b";
    assert_eq!(judge(text(nested), &ctx()), send(text("ab")));
}

#[test]
fn only_tool_calls_is_leaked() {
    assert_eq!(
        judge(text("<tool_call>x</tool_call>"), &ctx()),
        drop(OutWhy::Leaked)
    );
    let padded = " \n<function=f>1</function>\u{200B}<tool_call>2</tool_call>\n";
    assert_eq!(judge(text(padded), &ctx()), drop(OutWhy::Leaked));
}

#[test]
fn only_tool_calls_with_image_sends_the_image() {
    let leaky = with_images("<tool_call>x</tool_call>", &["img-a"]);
    assert_eq!(judge(leaky, &ctx()), send(with_images("", &["img-a"])));
}

#[test]
fn blank_and_invisible_is_blank() {
    let invisible = [
        "",
        " ",
        "\n\t ",
        "\u{200B}",
        "\u{200C}",
        "\u{200D}",
        "\u{200E}",
        "\u{200F}",
        "\u{2060}",
        "\u{2061}",
        "\u{2062}",
        "\u{2063}",
        "\u{2064}",
        "\u{FEFF}",
        "\u{00AD}",
        "\u{180E}",
        "\u{2028}",
        "\u{2029}",
        " \u{200B}\u{FEFF} \n\u{2060}",
    ];
    for blank in invisible {
        assert_eq!(judge(text(blank), &ctx()), drop(OutWhy::Blank), "{blank:?}");
    }
}

#[test]
fn visible_neighbours_of_invisible_ranges_are_kept() {
    // 那几段两边紧挨着的字是看得见的；零宽连接符夹在表情里也不是空的。
    for visible in [
        "\u{2010}",
        "\u{2065}",
        "\u{00AE}",
        "\u{180F}",
        "\u{2027}",
        "👨\u{200D}👩",
    ] {
        assert_eq!(
            judge(text(visible), &ctx()),
            send(text(visible)),
            "{visible:?}"
        );
    }
}

#[test]
fn blank_with_image_sends_the_image_without_text() {
    let blank = with_images(" \u{200B} ", &["img-a"]);
    assert_eq!(judge(blank, &ctx()), send(with_images("", &["img-a"])));
}

#[test]
fn parenthetical_aside_is_dropped() {
    for aside in ["（笑）", "  （小声）\n", "（偷偷（真的）看了一眼）", "（）"]
    {
        assert_eq!(judge(text(aside), &ctx()), drop(OutWhy::Aside), "{aside:?}");
    }
}

#[test]
fn text_outside_the_parentheses_is_not_an_aside() {
    for spoken in [
        "（笑）好的",
        "好的（笑）",
        "（笑）（哭）",
        "（笑",
        "笑）",
        "（笑））",
        "(笑)",
    ] {
        assert_eq!(
            judge(text(spoken), &ctx()),
            send(text(spoken)),
            "{spoken:?}"
        );
    }
}

#[test]
fn aside_with_image_is_kept() {
    let aside = with_images("（递给你）", &["img-a"]);
    assert_eq!(judge(aside.clone(), &ctx()), send(aside));
}

#[test]
fn exact_repeat_is_dropped() {
    let ctx = sent(&["别的", "好的，马上来"]);
    assert_eq!(judge(text("好的，马上来"), &ctx), drop(OutWhy::Repeated));
}

#[test]
fn repeat_ignores_punctuation_space_and_case() {
    let ctx = sent(&["Hello, World!"]);
    assert_eq!(judge(text("hello   world"), &ctx), drop(OutWhy::Repeated));
    assert_eq!(judge(text("HELLO… WORLD？"), &ctx), drop(OutWhy::Repeated));
    let ctx = sent(&["ÀB"]);
    assert_eq!(judge(text("àb"), &ctx), drop(OutWhy::Repeated));
}

#[test]
fn short_repeat_counts_too() {
    assert_eq!(judge(text("嗯"), &sent(&["嗯。"])), drop(OutWhy::Repeated));
}

#[test]
fn nothing_left_after_normalizing_is_never_a_repeat() {
    assert_eq!(
        judge(text("？？？"), &sent(&["！！！", "？？？"])),
        send(text("？？？"))
    );
}

#[test]
fn similarity_exactly_066_is_a_repeat() {
    // 共同的 34 个字给出 33 个共同的两字组；这一条多 8 个、发过的多 9 个：交集 33，并集 50，正好 0.66。
    let common = distinct(0, 34);
    let mine = format!("{common}{}", distinct(100, 8));
    let theirs = format!("{common}{}", distinct(200, 9));
    assert_eq!(
        judge(text(&mine), &sent(&[&theirs])),
        drop(OutWhy::Repeated)
    );
    // 发过的再多一个：并集 51，不到 0.66。
    let theirs = format!("{common}{}", distinct(200, 10));
    assert_eq!(judge(text(&mine), &sent(&[&theirs])), send(text(&mine)));
}

#[test]
fn fifteen_bigrams_skip_similarity() {
    // 这一条 16 个字 15 个两字组，全在发过的里头，也不比相似度。
    let mine = distinct(0, 16);
    let theirs = distinct(0, 17);
    assert_eq!(judge(text(&mine), &sent(&[&theirs])), send(text(&mine)));
    // 17 个字 16 个两字组：比，16 ÷ 17 够了。
    let mine = distinct(0, 17);
    let theirs = distinct(0, 18);
    assert_eq!(
        judge(text(&mine), &sent(&[&theirs])),
        drop(OutWhy::Repeated)
    );
}

#[test]
fn similarity_counts_bigrams_as_a_set() {
    // 两字组照集合数：重复的字不多算，「哈哈哈……」凑不够 16 个。
    let laugh = "哈".repeat(40);
    let ctx = sent(&[&"哈".repeat(39)]);
    assert_eq!(judge(text(&laugh), &ctx), send(text(&laugh)));
}

#[test]
fn only_this_turn_is_compared() {
    // 跨回合的不管：外面只交这一回合发过的，没发过就不算重复。
    assert_eq!(
        judge(text("好的，马上来"), &ctx()),
        send(text("好的，马上来"))
    );
}

#[test]
fn repeated_text_with_image_keeps_the_image() {
    let ctx = sent(&["画好了，拿去当壁纸吧"]);
    let outgoing = with_images("画好了，拿去当壁纸吧！", &["img-a"]);
    assert_eq!(judge(outgoing, &ctx), send(with_images("", &["img-a"])));
}

#[test]
fn repeated_images_are_removed() {
    let mut ctx = sent(&[]);
    ctx.sent.images = vec!["img-a".to_string()];
    let outgoing = with_images("再来一张", &["img-a", "img-b"]);
    assert_eq!(
        judge(outgoing, &ctx),
        send(with_images("再来一张", &["img-b"]))
    );
    let outgoing = with_images("还是这张", &["img-a"]);
    assert_eq!(judge(outgoing, &ctx), send(text("还是这张")));
}

#[test]
fn all_images_repeated_and_no_text_is_dropped() {
    let mut ctx = sent(&["画好了"]);
    ctx.sent.images = vec!["img-a".to_string()];
    assert_eq!(
        judge(with_images("", &["img-a"]), &ctx),
        drop(OutWhy::Repeated)
    );
    assert_eq!(
        judge(with_images("画好了", &["img-a"]), &ctx),
        drop(OutWhy::Repeated)
    );
}

#[test]
fn same_image_twice_in_one_keeps_the_first() {
    let outgoing = with_images("两张", &["img-b", "img-a", "img-b", "img-a"]);
    assert_eq!(
        judge(outgoing, &ctx()),
        send(with_images("两张", &["img-b", "img-a"]))
    );
}

/// 本来想引用也想 @，`others` 条别人的消息、过了 `elapsed` 毫秒、最后一条是不是她自己的：实际带不带。
fn target(others: u64, elapsed: i64, last_is_own: bool) -> Target {
    let mut ctx = ctx();
    ctx.target = Target {
        quote: true,
        mention: true,
    };
    ctx.since.others = others;
    ctx.since.elapsed = elapsed;
    ctx.since.last_is_own = last_is_own;
    match judge(text("好的"), &ctx) {
        Out::Send { target, .. } => target,
        Out::Drop(why) => panic!("dropped: {why:?}"),
    }
}

#[test]
fn quote_after_enough_others() {
    assert!(target(4, 0, false).quote);
    assert!(target(5, 0, false).quote);
    assert!(!target(3, 0, false).quote);
    assert!(!target(0, 0, false).quote);
}

#[test]
fn quote_when_last_message_is_her_own() {
    assert!(target(0, 0, true).quote);
}

#[test]
fn quote_after_zero_always_quotes() {
    let mut ctx = ctx();
    ctx.target.quote = true;
    ctx.outbound.quote_after = 0;
    let Out::Send { target, .. } = judge(text("好的"), &ctx) else {
        panic!("dropped");
    };
    assert!(target.quote);
}

#[test]
fn mention_needs_time_and_others() {
    assert!(target(1, MENTION_AFTER, false).mention);
    assert!(!target(1, MENTION_AFTER - 1, false).mention);
    assert!(!target(0, MENTION_AFTER * 10, true).mention);
    assert!(!target(5, -1, false).mention);
}

#[test]
fn unwanted_target_stays_off() {
    let mut ctx = ctx();
    ctx.since.others = 10;
    ctx.since.elapsed = MENTION_AFTER * 10;
    ctx.since.last_is_own = true;
    ctx.outbound.quote_after = 0;
    assert_eq!(judge(text("好的"), &ctx), send(text("好的")));
}
