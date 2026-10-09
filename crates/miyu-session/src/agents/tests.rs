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
