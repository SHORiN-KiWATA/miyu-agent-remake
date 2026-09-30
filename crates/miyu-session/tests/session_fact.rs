//! 会话编号那一块事实，执行器这一头（施工 1-13 再补，`docs/blueprint/kernel/request.md`「事实」）：造会话、载入时把会话自己的
//! 编号交给内核。第一轮注入的就是它，排在人那一句前面；载入以后编号一样，不重发；子会话注入它自己的编号，不是父会话的。

mod support;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::SessionId;
use miyu_kernel::request::Message;
use miyu_session::Lineage;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use support::*;

/// 日志里内核注入的会话编号那几块的原文，照先后。
fn session_facts(log: &[Event]) -> Vec<String> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ContextInjected(fact) if fact.kind.as_str() == "session" => {
                Some(fact.text.clone())
            }
            _ => None,
        })
        .collect()
}

/// 编号 `id` 那一块：出厂的模板，行尾一个换行。
fn block(id: &SessionId) -> String {
    format!("<session id=\"{id}\"/>\n")
}

/// 说一句 `words`，等这一轮说完。
async fn one_turn(handle: &miyu_session::Handle, command: &str, words: &str) {
    let mut pushes = watch(handle).await;
    ask(handle, command, say(words)).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

#[tokio::test]
async fn the_first_turn_carries_the_session_id_and_loading_does_not_send_it_again() {
    let home = Home::new();
    let before = Script::new([Play::Says("你好。")]);
    let handle = home.create(&before).await;
    let session = handle.id().clone();
    one_turn(&handle, "cmd-1", "hi").await;
    assert_eq!(session_facts(&home.log(&session)), [block(&session)]);
    // 第一次请求里，环境、权限、编号三块排在人那一句前面。
    let Some(Message::User { blocks }) = before.requests()[0].1.messages.first().cloned() else {
        panic!("第一条是人这一边的");
    };
    let texts: Vec<String> = blocks
        .into_iter()
        .map(|block| match block {
            Block::Text(Text { text }) => text,
            other => panic!("都是字：{other:?}"),
        })
        .collect();
    assert_eq!(texts.len(), 4, "{texts:?}");
    assert_eq!(texts[2], block(&session));
    assert_eq!(texts[3], "hi");
    stop(&handle).await;

    let after = Script::new([Play::Says("再见。")]);
    let handle = home.load(&session, &after).await;
    one_turn(&handle, "cmd-2", "bye").await;
    assert_eq!(
        session_facts(&home.log(&session)),
        [block(&session)],
        "载入以后编号一样，不重发"
    );
}

#[tokio::test]
async fn a_child_session_carries_its_own_id() {
    let home = Home::new();
    let parent = SessionId::parse("01a0d75d-2180-7a3c-9e41-5b7d2c8f6a10").unwrap();
    let script = Script::new([Play::Says("好。")]);
    let lines = Lines {
        lineage: Some(Lineage {
            parent: parent.clone(),
            depth: 1,
        }),
        ..Lines::default()
    };
    let handle = home
        .create_full(&script, &Catalog::default(), Opening::default(), lines)
        .await;
    let session = handle.id().clone();
    assert_ne!(session, parent);
    one_turn(&handle, "cmd-1", "做这件事").await;
    assert_eq!(session_facts(&home.log(&session)), [block(&session)]);
}
