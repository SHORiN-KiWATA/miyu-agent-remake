//! 场所规则套到这一条上（`onebot.md` 第一条「群消息」第 3、4、6 条）：人格、预设、工作区照规则带、没设的不带；`managers`
//! 里有的是管理的人；`show_ids`；睡觉时间盖住此刻的睡着（照本机的时区），没盖住、`off`、没设的醒着。

use std::path::Path;

use miyu_chat::{File, Params, Rules, Source, Venue, VenueKind};

use super::{asleep, opening, role, show_ids};
use crate::onebot::person;
use crate::rules::{Applied, Loaded};

/// 只有一份系统的规则文件 `text` 时，套到群 555 上的样子；参数照出厂的 `defaults.toml`。
fn applied(text: &str) -> Applied {
    let defaults =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/software/onebot/defaults.toml");
    let params = Params::read(&File {
        source: Source::Factory,
        name: "defaults.toml".to_string(),
        text: std::fs::read_to_string(defaults).expect("出厂参数读得了"),
    })
    .expect("出厂参数读得出");
    let rules = File {
        source: Source::System,
        name: "80-test.toml".to_string(),
        text: format!("[[rule]]\n{text}"),
    };
    let parsed = Rules::parse(&[rules]);
    assert!(parsed.problems.is_empty(), "{:?}", parsed.problems);
    let loaded = Loaded {
        rules: parsed.rules,
        params,
        keywords: Vec::new(),
        problems: Vec::new(),
    };
    loaded.at(&Venue::new("qq", VenueKind::Group, "555").expect("拼得出"))
}

/// 一天里的第 `minute` 分钟写成 `HH:MM`（越界的绕回来）。
fn clock(minute: i64) -> String {
    let minute = minute.rem_euclid(24 * 60);
    format!("{:02}:{:02}", minute / 60, minute % 60)
}

/// 本机此刻是一天里的第几分钟。
fn this_minute() -> i64 {
    let now = jiff::Zoned::now();
    i64::from(now.hour()) * 60 + i64::from(now.minute())
}

#[test]
fn the_opening_carries_what_the_rules_set() {
    let set = applied("persona = \"miyu\"\npreset = \"chat\"\nworkspace = \"/srv/group\"\n");
    assert_eq!(
        opening(&set),
        [
            ("persona", "miyu"),
            ("preset", "chat"),
            ("cwd", "/srv/group")
        ]
    );
    assert_eq!(
        opening(&applied("preset = \"chat\"\n")),
        [("preset", "chat")]
    );
    assert!(opening(&applied("rate = \"0\"\n")).is_empty(), "没设的不带");
}

#[test]
fn managers_and_show_ids_follow_the_rules() {
    let set = applied("managers = [\"qq:40004\", \"tg:20002\"]\nshow_ids = true\n");
    assert_eq!(role(&set, &person(40004).expect("拼得出")), "manager");
    assert_eq!(
        role(&set, &person(20002).expect("拼得出")),
        "member",
        "别的平台的同一个号不算"
    );
    assert!(show_ids(&set));
    let unset = applied("rate = \"0\"\n");
    assert_eq!(role(&unset, &person(40004).expect("拼得出")), "member");
    assert!(!show_ids(&unset));
    assert!(!show_ids(&applied("show_ids = false\n")));
}

#[test]
fn asleep_is_when_the_sleep_rule_covers_now() {
    let now = this_minute();
    let covering = format!("sleep = \"{}-{}\"\n", clock(now - 60), clock(now + 60));
    assert!(asleep(&applied(&covering)), "{covering}");
    let later = format!("sleep = \"{}-{}\"\n", clock(now + 120), clock(now + 180));
    assert!(!asleep(&applied(&later)), "{later}");
    assert!(!asleep(&applied("sleep = \"off\"\n")));
    assert!(!asleep(&applied("rate = \"0\"\n")), "没设的醒着");
}
