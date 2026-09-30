//! 请求形状探针：人切了权限级别的会话（施工 2-7 补，`docs/blueprint/kernel/request.md`「事实」第 2 条）。真内核照剧本跑：
//! 第一轮在工作区；空闲时切到完全放开，第二轮开头用切换那一份告诉她；第三轮工具在跑时开了只读又关掉，不注入；第四轮工具在跑
//! 时开只读，这一轮下一次请求之前告诉她。每一次请求和存档（`docs/designs/samples/probe/permission/`）逐字节比，查五条性质；
//! 和同一份剧本在以前造的快照（没有切换那一份）上跑的比，每一次请求只差切换那几块：换回平常那一份就一字不差。

mod support;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Level;
use miyu_kernel::facts::FactTemplates;
use miyu_kernel::request::{Message, Request};
use miyu_kernel::session::Policy;
use miyu_kernel::testkit::{Line, Play, Stage};
use support::{check, files, matches_the_archive, policy, sent, stage, stage_with};

/// 平常那一份的原文。
const PLAIN: &str = include_str!("../../../resources/core/facts/permission.txt");
/// 切换那一份的原文。
const CHANGED: &str = include_str!("../../../resources/core/facts/permission-changed.txt");
/// 权限那一块写得出的几级。
const LEVELS: [&str; 3] = ["read_only", "workspace", "full"];

/// 以前造的快照：出厂的模板少了切换那一份，别的和 [`policy`] 一样。
fn older() -> Policy {
    let mut policy = policy();
    policy.facts = FactTemplates::new(
        include_str!("../../../resources/core/facts/env.txt"),
        PLAIN,
        include_str!("../../../resources/core/facts/reply-cut.txt"),
        Some(include_str!("../../../resources/core/facts/session.txt")),
        None,
    )
    .expect("出厂的模板用得了");
    policy
}

/// 剧本，在替身 `s` 上跑。
fn script(mut s: Stage) -> Stage {
    // 1. 第一轮在工作区：环境、权限、会话编号三块事实。
    s.model([Line::says("src 下有 lib.rs 和 main.rs。")]);
    s.say("看看 src 目录");

    // 2. 空闲时切到完全放开，再说一句：第二轮开头告诉她切了。
    s.advance(2);
    s.set_permission(Some(Level::Full), None);
    s.model([
        Line::calls("好，我写。", &[("write", r#"{"path":"NOTES.md"}"#)]),
        Line::says("写好了。"),
    ]);
    s.tools([Play::done("Created NOTES.md")]);
    s.say("现在呢");

    // 3. 她读文件的时候开了只读又关掉：到下一次请求时级别和她看到的一样，不注入。
    s.advance(1);
    s.model([
        Line::calls("我读一下。", &[("read", r#"{"path":"src/lib.rs"}"#)]),
        Line::says("lib.rs 只有一行。"),
    ]);
    s.tools([Play::done("pub mod assemble;").held()]);
    s.say("读一下 lib.rs");
    let reading = s.ran().last().expect("派了读").0;
    s.set_permission(None, Some(true));
    s.set_permission(None, Some(false));
    s.release_tool(reading);

    // 4. 她读文件的时候开只读：这一轮下一次请求之前告诉她，排在工具结果后面。
    s.advance(1);
    s.model([
        Line::calls("我读一下。", &[("read", r#"{"path":"src/main.rs"}"#)]),
        Line::says("现在是只读，我只看不改。"),
    ]);
    s.tools([Play::done("fn main() {}").held()]);
    s.say("读一下 main.rs，再改成打印 hello");
    let reading = s.ran().last().expect("派了读").0;
    s.set_permission(None, Some(true));
    s.release_tool(reading);
    s
}

/// 切换那一份写出的从 `previous` 切到 `level`。
fn changed(level: &str, previous: &str) -> String {
    CHANGED
        .replace("{level}", level)
        .replace("{previous}", previous)
}

/// 切换那几块换回平常那一份，交回换了几块。
fn plain_again(request: &Request) -> (Request, usize) {
    let mut request = request.clone();
    let mut swapped = 0;
    for message in &mut request.messages {
        let Message::User { blocks } = message else {
            continue;
        };
        for block in blocks {
            let Block::Text(Text { text }) = block else {
                continue;
            };
            let level = LEVELS.into_iter().find(|level| {
                LEVELS
                    .into_iter()
                    .any(|previous| *text == changed(level, previous))
            });
            if let Some(level) = level {
                *text = PLAIN.replace("{level}", level);
                swapped += 1;
            }
        }
    }
    (request, swapped)
}

#[test]
fn the_permission_session_matches_the_archive() {
    matches_the_archive("permission", &script(stage()));
}

#[test]
fn the_same_script_gives_the_same_bytes() {
    assert_eq!(files(&script(stage())), files(&script(stage())));
}

#[test]
fn the_permission_session_keeps_the_properties() {
    let session = script(stage());
    if let Err(why) = check(&sent(&session)) {
        panic!("{why}");
    }
    let requests = session.requests();
    assert_eq!(requests.len(), 7);
    // 第二轮的第一次请求：最后一条 user 是切换那一块和那句话。
    let Some(Message::User { blocks }) = requests[1].1.messages.last() else {
        panic!("最后一条是 user");
    };
    assert_eq!(
        blocks.as_slice(),
        [
            Block::Text(Text {
                text: changed("full", "workspace")
            }),
            Block::Text(Text {
                text: "现在呢".to_string()
            }),
        ]
    );
    // 第四轮的第二次请求：切换那一块单独一条 user，排在工具结果后面。
    let last = &requests[6].1;
    assert!(
        matches!(last.messages.last(), Some(Message::User { blocks })
            if blocks.as_slice() == [Block::Text(Text { text: changed("read_only", "full") })]),
        "{:?}",
        last.messages.last()
    );
    assert!(matches!(
        last.messages.iter().rev().nth(1),
        Some(Message::Tool { .. })
    ));
}

/// 以前造的快照上跑同一份剧本：每一次请求只差切换那几块，换回平常那一份一字不差。第一次请求一块都没有；第二次起带着
/// 第二轮开头那一块，最后一次还带着第四轮中途那一块。
#[test]
fn it_differs_from_an_older_snapshot_only_by_the_changed_blocks() {
    let (now, older) = (script(stage()), script(stage_with(older)));
    assert_eq!(now.log().len(), older.log().len());
    assert_eq!(now.requests().len(), older.requests().len());
    let mut counts = Vec::new();
    for ((_, new), (_, old)) in now.requests().iter().zip(older.requests()) {
        let (plain, swapped) = plain_again(new);
        assert_eq!(plain, *old, "换回平常那一份就是以前的那一份");
        counts.push(swapped);
    }
    assert_eq!(counts, [0, 1, 1, 1, 1, 1, 2]);
}
