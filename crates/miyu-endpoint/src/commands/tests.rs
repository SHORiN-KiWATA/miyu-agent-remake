//! 认命令、判谁能用（施工 O-6）：协议上走不到的几种在这里守，例如群里的主人（对应表里有、又不是会话属主的外部身份，要等
//! 有第二个账号才造得出来）。

use miyu_kernel::id::{AccountId, ExternalId, VenueId};
use miyu_kernel::origin::{By, External, Person, Role};

use serde_json::{Value, json};

use miyu_kernel::id::CommandId;

use super::{Slash, answered_later, may_run, parse};
use crate::wire::Request;

fn external(account: Option<&str>, role: Option<Role>) -> By {
    By::External(External {
        venue: VenueId::parse("qq:group:1").expect("合写法"),
        id: ExternalId::parse("qq:10001").expect("合写法"),
        account: account.map(|name| AccountId::parse(name).expect("合写法")),
        role,
    })
}

#[test]
fn the_owner_and_managers_may_run_and_others_may_not() {
    let alice = AccountId::parse("alice").expect("合写法");
    assert!(may_run(&By::Person(Person::new(alice))));
    assert!(
        may_run(&external(Some("alice"), Some(Role::Member))),
        "群里的主人"
    );
    assert!(may_run(&external(None, Some(Role::Manager))));
    assert!(!may_run(&external(None, Some(Role::Member))));
    assert!(!may_run(&external(None, None)));
    assert!(!may_run(&By::Kernel));
}

#[test]
fn names_and_aliases() {
    assert_eq!(parse("/clear").ok(), Some((Slash::Clear, "")));
    assert_eq!(parse("/reset").ok(), Some((Slash::Clear, "")));
    assert_eq!(parse("\t/stop now").ok(), Some((Slash::Stop, "now")));
    assert_eq!(parse("/stop\nnow").ok(), Some((Slash::Stop, "now")));
    assert_eq!(
        parse("/workspace  ~/a b \n").ok(),
        Some((Slash::Workspace, "~/a b"))
    );
    assert_eq!(
        parse("/remember  用户 喜欢猫 ").ok(),
        Some((Slash::Remember, "用户 喜欢猫"))
    );
    assert_eq!(parse("/remember").ok(), Some((Slash::Remember, "")));
    assert_eq!(
        parse("/Stop").err().map(|refusal| refusal.reason),
        Some("unknown_command")
    );
    assert_eq!(
        parse("stop").err().map(|refusal| refusal.reason),
        Some("bad_params")
    );
}

#[test]
fn only_dream_is_answered_later() {
    let run = |text: Value| Request {
        id: CommandId::parse("k1").expect("合写法"),
        method: "command.run".to_string(),
        params: json!({"session": "s", "text": text}),
    };
    assert!(answered_later(&run(json!("/dream"))));
    assert!(answered_later(&run(json!("  /dream 现在"))));
    for text in [
        json!("/clear"),
        json!("/reset"),
        json!("/stop"),
        json!("/remember 用户喜欢猫"),
        json!("/workspace ~"),
        json!("/dreams"),
        json!("dream"),
        json!(3),
    ] {
        assert!(!answered_later(&run(text.clone())), "{text}");
    }
    let no_text = Request {
        params: json!({"session": "s"}),
        ..run(json!(""))
    };
    assert!(!answered_later(&no_text));
    assert_eq!(parse("/dream").ok(), Some((Slash::Dream, "")));
}
