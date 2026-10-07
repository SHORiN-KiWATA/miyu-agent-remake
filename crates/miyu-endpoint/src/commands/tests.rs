//! 认命令、判谁能用（施工 O-6）：协议上走不到的几种在这里守，例如群里的主人（对应表里有、又不是会话属主的外部身份，要等
//! 有第二个账号才造得出来）。

use miyu_kernel::id::{AccountId, ExternalId, VenueId};
use miyu_kernel::origin::{By, External, Person, Role};

use super::{Slash, may_run, parse};

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
    assert_eq!(parse("/clear").ok(), Some(Slash::Clear));
    assert_eq!(parse("/reset").ok(), Some(Slash::Clear));
    assert_eq!(parse("\t/stop now").ok(), Some(Slash::Stop));
    assert_eq!(parse("/stop\nnow").ok(), Some(Slash::Stop));
    assert_eq!(
        parse("/Stop").err().map(|refusal| refusal.reason),
        Some("unknown_command")
    );
    assert_eq!(
        parse("stop").err().map(|refusal| refusal.reason),
        Some("bad_params")
    );
}
