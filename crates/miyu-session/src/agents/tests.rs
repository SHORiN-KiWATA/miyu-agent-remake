//! 工具面照会话在哪挑提供者的工具（施工 O-2 上）：给本机的只进本机的会话，给私聊、群的只进那一种场所会话；核心自带的照旧。

use miyu_config::Values;
use miyu_kernel::tool::Access;
use miyu_tool::Venues;
use miyu_tool::testkit::{Act, Fake};

use super::*;

fn catalog() -> Catalog {
    let only = |local, private, group| Venues {
        local,
        private,
        group,
    };
    Catalog::in_packages([
        (
            "basesystem",
            vec![Fake::new("read", Access::Read, Act::Echo) as Arc<dyn miyu_tool::Tool>],
        ),
        (
            "onebot",
            vec![
                Fake::in_venues(
                    "send_any",
                    Access::Outbound,
                    only(true, false, false),
                    Act::Echo,
                ),
                Fake::in_venues(
                    "skip_reply",
                    Access::Read,
                    only(false, true, true),
                    Act::Echo,
                ),
                Fake::in_venues(
                    "mute",
                    Access::Outbound,
                    only(false, false, true),
                    Act::Echo,
                ),
            ],
        ),
    ])
    .expect("合写法")
}

fn names(venue: &str, group: bool) -> Vec<String> {
    let venue = VenueId::parse(venue).expect("合写法");
    Agents::face(
        &catalog(),
        Site {
            venue: &venue,
            group,
        },
        None,
        &Offers::of(&Values::default(), Vec::new()),
        false,
        MemoryScope::Off,
        None,
    )
    .into_iter()
    .map(|entry| entry.name)
    .collect()
}

#[test]
fn provided_tools_go_to_the_kind_of_session_they_are_for() {
    assert_eq!(names("local", false), ["read", "send_any"]);
    assert_eq!(names("qq:private:10001", false), ["read", "skip_reply"]);
    assert_eq!(names("qq:group:1", true), ["mute", "read", "skip_reply"]);
}

/// 结果没人要了时停哪几个（施工 7-5 补）：派出去的子代理，后台命令、没有子会话的不算。
#[test]
fn only_spawned_subagents_count_as_unclaimed() {
    let job = |n: &str| JobId::parse(n).expect("合写法");
    let child = SessionId::parse("01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91").expect("合写法");
    let started = |n: &str, what: JobKind, session: Option<SessionId>| {
        Effect::JobStarted(JobStarted {
            foreground: false,
            job: job(n),
            what,
            title: "查".to_string(),
            session,
        })
    };
    let effects = [
        started("j1", JobKind::Command, None),
        started("j2", JobKind::Agent, Some(child.clone())),
        started("j3", JobKind::Agent, None),
    ];
    assert_eq!(spawned_in(&effects), [(job("j2"), child)]);
    assert!(spawned_in(&[]).is_empty());
}
