//! 闲够了的会话退下（施工 V-2 再补，`docs/blueprint/session/actor.md`「退下」）：会话表问，actor 照自己的账答。别的都空着、
//! 闲得不够的答还差多久；有头看着、有派出去还没回报的、有会话等它空下来、它在等别的会话、记忆的闹钟上着的答还有事；什么都
//! 没有、闲够了的退下，之后再交命令是「会话停了」，载入回来接着用。

use std::sync::Arc;
use std::time::Duration;

use miyu_http::testkit::Server;
use miyu_kernel::event::Body;
use miyu_kernel::id::SessionId;
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_session::{Handle, Retire, Stopped};
use miyu_tool::testkit::{Act, Fake, Held};
use miyu_tool::{Catalog, Exit, Tool};

use crate::support::extracting::organizer_after;
use crate::support::meaning::{catalog, chat};
use crate::support::table::{OTHER, Table, WAITER, session, sid};
use crate::support::*;

/// 问退不退时说的「闲多久」：长得这次测试里不会到。
const LONG: Duration = Duration::from_secs(3600);

/// 照 `plays` 回，起标题的请求回一个标题：没排标题的，假模型不回它，那个请求一直在路上，会话就一直有事。
fn script(plays: impl IntoIterator<Item = Play>) -> Script {
    Script::new(plays).titles([Play::Says("测试")])
}

/// 说一句，等这一轮结束，放下订阅。
async fn talk(handle: &Handle, id: &str, words: &str) {
    let mut pushes = watch(handle).await;
    ask(handle, id, say(words)).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

/// 一直问（闲 [`LONG`] 才退）到不再答「还有事」：一轮结束以后，起标题这类辅助请求还在路上一会儿。
async fn settled(handle: &Handle) -> Retire {
    within("不再答还有事", async {
        loop {
            match handle.retire(LONG).await.expect("会话在跑") {
                Retire::Kept => tokio::time::sleep(Duration::from_millis(5)).await,
                answer => return answer,
            }
        }
    })
    .await
}

/// 问 `times` 次、每次隔一会儿，闲多久都算够，一次都没退下。
async fn kept(handle: &Handle, times: usize) {
    for _ in 0..times {
        assert_eq!(handle.retire(Duration::ZERO).await, Ok(Retire::Kept));
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 日志里结束了几轮。
fn ended(home: &Home, session: &SessionId) -> usize {
    home.log(session)
        .iter()
        .filter(|event| matches!(event.body, Body::TurnEnded(_)))
        .count()
}

#[tokio::test]
async fn a_quiet_session_retires_once_idle_long_enough_and_loads_again() {
    let home = Home::new();
    let handle = home.create(&script([Play::Says("在。")])).await;
    talk(&handle, "m1", "在吗").await;
    let Retire::Later(left) = settled(&handle).await else {
        panic!("别的都空着，只差闲得不够久");
    };
    assert!(
        left <= LONG && left > LONG - Duration::from_secs(60),
        "{left:?}"
    );
    assert_eq!(handle.retire(Duration::ZERO).await, Ok(Retire::Retired));
    assert_eq!(
        handle.command(id("m2"), alice(), say("还在吗")).await,
        Err(Stopped),
        "退下了，不再收"
    );
    let session = handle.id().clone();
    drop(handle);
    let handle = home.load(&session, &script([Play::Says("还在。")])).await;
    talk(&handle, "m2", "还在吗").await;
    assert_eq!(ended(&home, &session), 2, "载入回来接着用");
}

#[tokio::test]
async fn a_watched_session_is_kept_until_the_head_goes() {
    let home = Home::new();
    let handle = home.create(&script([Play::Says("在。")])).await;
    talk(&handle, "m1", "在吗").await;
    assert!(matches!(settled(&handle).await, Retire::Later(_)));
    let pushes = watch(&handle).await;
    kept(&handle, 3).await;
    drop(pushes);
    assert_eq!(handle.retire(Duration::ZERO).await, Ok(Retire::Retired));
}

#[tokio::test]
async fn a_running_background_command_keeps_it_until_reported() {
    let home = Home::new();
    let held = Held::new(&[]);
    let tool = Fake::new("start", Access::Read, Act::Background(Arc::clone(&held)));
    let tools = Catalog::new([tool as Arc<dyn Tool>]).expect("合写法");
    let plays = script([
        Play::calls(&[("start", "{}")]),
        Play::Says("放出去了。"),
        Play::Says("结束了。"),
    ]);
    let handle = home.create_with(&plays, &tools).await;
    talk(&handle, "m1", "放一个").await;
    kept(&handle, 10).await;
    held.end(Exit::Code(0));
    until_logged(&home, handle.id(), |log| {
        log.iter()
            .filter(|event| matches!(event.body, Body::TurnEnded(_)))
            .count()
            == 2
    })
    .await;
    assert!(matches!(settled(&handle).await, Retire::Later(_)));
    assert_eq!(handle.retire(Duration::ZERO).await, Ok(Retire::Retired));
}

#[tokio::test]
async fn a_session_another_waits_on_is_kept() {
    let home = Home::new();
    let table = Arc::new(Table::default());
    let handle = session(&home, &script([Play::Says("在。")]), &table).await;
    talk(&handle, "m1", "在吗").await;
    assert!(matches!(settled(&handle).await, Retire::Later(_)));
    // 起算时刻晚于它忙完的那一刻：同一毫秒的当「带话又订」，当场发、不留在名单上。
    tokio::time::sleep(Duration::from_millis(20)).await;
    handle.watch(sid(WAITER), now()).expect("会话在跑");
    kept(&handle, 3).await;
    assert!(table.notices().is_empty(), "空着订进来的，等它下一次忙完");
}

#[tokio::test]
async fn a_session_waiting_on_another_is_kept() {
    let home = Home::new();
    let table = Arc::new(Table::default());
    let args = serde_json::json!({"to": OTHER, "notify_when_idle": true}).to_string();
    let plays = script([
        Play::calls(&[("send_message", args.as_str())]),
        Play::Says("等它。"),
    ]);
    let handle = session(&home, &plays, &table).await;
    talk(&handle, "m1", "它空了告诉我").await;
    until_logged(&home, handle.id(), |_| !table.placed().is_empty()).await;
    kept(&handle, 10).await;
}

#[tokio::test]
async fn an_armed_memory_alarm_keeps_it() {
    let server = Server::start(Vec::new()).await;
    let mut home = Home::new();
    organizer_after(&mut home, &server, "", LONG);
    let handle = home
        .create_full(
            &script([Play::Says("好。"), Play::Says("好。")]),
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    chat(&home, &handle, 1, "我养了一只猫").await;
    chat(&home, &handle, 2, "它叫团子").await;
    kept(&handle, 10).await;
    assert!(server.received().is_empty(), "闹钟还没响");
}

#[tokio::test]
async fn idle_counts_from_the_last_mail() {
    let home = Home::new();
    let handle = home.create(&script([Play::Says("在。")])).await;
    talk(&handle, "m1", "在吗").await;
    assert!(matches!(settled(&handle).await, Retire::Later(_)));
    tokio::time::sleep(Duration::from_millis(300)).await;
    handle.current().await.expect("会话在跑");
    let idle = Duration::from_millis(250);
    assert!(
        matches!(handle.retire(idle).await, Ok(Retire::Later(_))),
        "刚收过一封，从那一刻起算"
    );
}

#[tokio::test]
async fn a_request_still_on_its_way_keeps_it() {
    let home = Home::new();
    // 起标题的请求没排回答：假模型不回它，一直在路上。
    let handle = home.create(&Script::new([Play::Says("在。")])).await;
    talk(&handle, "m1", "在吗").await;
    kept(&handle, 10).await;
}
