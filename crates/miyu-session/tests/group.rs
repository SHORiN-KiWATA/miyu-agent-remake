//! 群会话，执行器这一头（施工 O-13 中，`docs/construction/O-13-群里的一行和格式说明（中）.md`）：造的时候 system 在人设后面接上
//! 格式说明、快照钉下这时的时区；群里的人说的进请求是一行一条；换了时区的机器上载入，前面那一行一字不变、后面的也照钉下的
//! 时区；私聊的没有说明、快照里没有这一格，人的话照原样。

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Event, VenueMessage};
use miyu_kernel::facts::Environment;
use miyu_kernel::id::{ExternalId, SessionId, VenueId};
use miyu_kernel::origin::{By, External, Role};
use miyu_kernel::request::{Message, Request};
use miyu_kernel::session::Command;
use miyu_kernel::time::UtcOffset;
use miyu_policy::Snapshot;
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_store::blob::Blobs;
use miyu_tool::Catalog;

use crate::support::*;

const NOTE: &str = include_str!("../../../resources/core/venues/group.txt");

/// 场所 `venue` 的会话，`group` 的是群会话。
fn lines(venue: &str, group: bool) -> Lines {
    Lines {
        venue: VenueId::parse(venue).expect("场所合写法"),
        group,
        ..Lines::default()
    }
}

/// 群里的小林（`qq:20017`）。
fn member() -> By {
    By::External(External {
        venue: VenueId::parse("qq:group:1").expect("场所合写法"),
        id: ExternalId::parse("qq:20017").expect("身份合写法"),
        account: None,
        role: Some(Role::Member),
    })
}

/// 小林说 `words`，平台的编号是 `msg`。
fn heard(words: &str, msg: &str) -> Command {
    Command::Send {
        blocks: vec![Block::Text(Text {
            text: words.to_string(),
        })],
        urgent: false,
        venue: Some(VenueMessage {
            msg: msg.to_string(),
            name: Some("小林".to_string()),
            ..VenueMessage::default()
        }),
    }
}

/// `by` 说一句，等这一轮说完。
async fn one_turn(handle: &Handle, command: &str, by: By, said: Command) {
    let mut pushes = watch(handle).await;
    within("回应", handle.command(id(command), by, said))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

/// 会话 `session` 的策略快照。
fn snapshot(home: &Home, session: &SessionId) -> Snapshot {
    let log = home.log(session);
    let Body::SessionCreated(created) = &log[0].body else {
        panic!("第 1 条应该是造会话：{:?}", log[0]);
    };
    let bytes = Blobs::new(home.root.blobs(&alice_account()))
        .get(&created.policy)
        .expect("快照在 blob 里");
    Snapshot::from_bytes(&bytes).expect("读得懂")
}

/// 请求里人这一边的字，照先后。
fn said(request: &Request) -> Vec<String> {
    request
        .messages
        .iter()
        .filter_map(|message| match message {
            Message::User { blocks } => Some(blocks),
            _ => None,
        })
        .flatten()
        .filter_map(|block| match block {
            Block::Text(Text { text }) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// 日志里平台编号是 `msg` 的那一条。
fn message<'a>(log: &'a [Event], msg: &str) -> &'a Event {
    log.iter()
        .find(|event| {
            matches!(&event.body, Body::MessageUser(message)
                if message.venue.as_ref().is_some_and(|venue| venue.msg == msg))
        })
        .expect("记下了")
}

fn offset(minutes: i32) -> UtcOffset {
    UtcOffset::from_minutes(minutes).expect("在范围里")
}

#[tokio::test]
async fn a_group_has_the_note_a_pinned_clock_and_one_line_per_message() {
    let home = Home::new();
    let script = Script::new([Play::Says("我来。"), Play::Says("好。")]);
    let handle = home
        .create_full(
            &script,
            &Catalog::default(),
            Opening::default(),
            lines("qq:group:1", true),
        )
        .await;
    let session = handle.id().clone();
    let made = snapshot(&home, &session);
    assert!(made.system.contains(NOTE.trim_end()), "{}", made.system);
    assert_eq!(made.group.as_ref().map(|chat| chat.offset), Some(540));

    one_turn(&handle, "cmd-1", member(), heard("今天谁值班", "8810")).await;
    stop(&handle).await;
    let tokyo = offset(540);
    let first = message(&home.log(&session), "8810").at.local_clock(tokyo);
    let first = format!("[{first}] 小林 [msg=8810]: 今天谁值班");
    assert_eq!(said(&script.requests()[0].1).last(), Some(&first));

    // 换到西五区的机器上载入：前面那一行一字不变，新的一行也照东九区写。
    let west = Environment {
        offset: offset(-300),
        ..environment()
    };
    let handle = home
        .load_in(&session, &script, &Catalog::default(), west, None)
        .await;
    one_turn(&handle, "cmd-2", member(), heard("那我来", "8811")).await;
    stop(&handle).await;
    let second = message(&home.log(&session), "8811").at.local_clock(tokyo);
    let second = format!("[{second}] 小林 [msg=8811]: 那我来");
    let words = said(&script.requests()[1].1);
    assert!(words.contains(&first), "{words:?}");
    assert_eq!(words.last(), Some(&second));
}

#[tokio::test]
async fn a_private_chat_has_no_note_and_is_said_as_written() {
    let home = Home::new();
    let script = Script::new([Play::Says("在。")]);
    let handle = home
        .create_full(
            &script,
            &Catalog::default(),
            Opening::default(),
            lines("qq:private:20017", false),
        )
        .await;
    let session = handle.id().clone();
    let made = snapshot(&home, &session);
    assert!(!made.system.contains(NOTE.trim_end()));
    assert_eq!(made.group, None);
    one_turn(&handle, "cmd-1", member(), heard("在吗", "8820")).await;
    stop(&handle).await;
    assert_eq!(
        said(&script.requests()[0].1).last().map(String::as_str),
        Some("在吗")
    );
}
