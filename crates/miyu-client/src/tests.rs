//! 门面上的每一样都在（设计 32 第二节第 2 条）：照两个头 2026-10-11 报的用法列一遍，编得过就是在。改了、少了门面上的
//! 东西，这里跟着改，照改协议的规矩告诉两个头。

#[expect(unused_imports, reason = "只为钉住门面：拿掉了哪一样，这里当场编不过")]
use crate::{
    CoreCommand,
    connect::{
        ConnectError, Connection, Ready, StartError, connect, connect_or_start,
        connect_or_start_bare, spawn_detached,
    },
    log::{LevelFilter, install},
    manifest::{Value, read},
    open::{Browser, Core, SystemBrowser},
    places::{DataRoot, Env, Platform, ResourceRoot, cache_root, locale},
    protocol::{Said, SessionId, Template},
    texts::{Face, Human, clean},
};

/// 门面里的 `open`、`CoreCommand` 是挪来的那一份：网页、接入QQ 都照它连核心。
#[test]
fn the_face_lists_what_the_heads_use() {
    let kinds = [
        std::any::type_name::<crate::open::Core>(),
        std::any::type_name::<crate::CoreCommand>(),
    ];
    assert!(kinds[0].starts_with("miyu_client::open::"), "{kinds:?}");
}
