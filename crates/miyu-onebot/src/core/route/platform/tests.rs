//! 平台工具（一）做不做、对谁做（施工 O-31，`onebot.md` 第一条「平台工具（一）」第 3、4 条）：往坏里测。冒充（叫她做的那条和
//! `by` 对不上）、目标是她自己、引用和 @ 是同一个人、@ 了两个人、动不得的人、秒数的边界、私聊不分谁的；时长的写法。

use miyu_kernel::id::ExternalId;
use serde_json::json;

use super::{Caller, Order, Origin, Plan, Quote, Scene, Why, duration, plan};

/// 平台上号是 `number` 的人。
fn qq(number: i64) -> ExternalId {
    ExternalId::parse(&format!("qq:{number}")).expect("合写法")
}

/// 她自己。
const ME: i64 = 30003;

/// 终端管理员（投影里认得出）。
const BOSS: i64 = 10001;

/// 白名单成员。
const FRIEND: i64 = 50005;

/// 群里的情形：白名单照 [`FRIEND`]，终端管理员是 [`BOSS`]。
fn group<'a>(me: &'a ExternalId, whitelist: &'a [String]) -> Scene<'a> {
    Scene {
        peer: None,
        me,
        whitelist,
        admin: &|who: &ExternalId| *who == qq(BOSS),
    }
}

/// `number` 叫的，管理的人是 `manager`。
fn caller(number: i64, manager: bool) -> Caller {
    Caller {
        id: Some(qq(number)),
        manager,
    }
}

/// `number` 说的一条：引用 `quote`、@ 了 `mentions`。
fn origin(number: i64, quote: Option<Quote>, mentions: &[i64]) -> Origin {
    Origin {
        sender: qq(number),
        quote,
        mentions: mentions.iter().map(|one| qq(*one)).collect(),
    }
}

/// 引用平台编号 `msg` 那一条：她的是 `mine`，发的人是 `sender`。
fn quote(msg: &str, mine: bool, sender: Option<i64>) -> Option<Quote> {
    Some(Quote {
        msg: msg.to_string(),
        mine,
        sender: sender.map(qq),
    })
}

/// 在群里定。
fn in_group(order: Order, caller: &Caller, origin: Option<&Origin>) -> Plan {
    let me = qq(ME);
    let whitelist = [format!("qq:{FRIEND}")];
    plan(order, caller, origin, &group(&me, &whitelist))
}

#[test]
fn orders_are_read_from_the_tool_and_seconds_only_in_range() {
    assert_eq!(Order::read("recall", &json!({})), Some(Order::Recall));
    assert_eq!(Order::read("poke", &json!({})), Some(Order::Poke));
    assert_eq!(Order::read("skip_reply", &json!({})), None);
    for (args, wanted) in [
        (json!({"seconds": 0}), Some(0)),
        (json!({"seconds": 600}), Some(600)),
        (json!({"seconds": 2_592_000}), Some(2_592_000)),
        (json!({"seconds": 2_592_001}), None),
        (json!({"seconds": -1}), None),
        (json!({"seconds": 1.5}), None),
        (json!({"seconds": "600"}), None),
        (json!({}), None),
    ] {
        assert_eq!(
            Order::read("mute", &args),
            Some(Order::Mute(wanted)),
            "{args}"
        );
    }
}

#[test]
fn the_caller_is_a_manager_by_the_owner_flag_or_the_venue_role_only() {
    let by = |role: Option<&str>, owner: bool| {
        let mut by = json!({"kind": "external", "venue": "qq:group:1", "id": "qq:40001"});
        if let Some(role) = role {
            by["role"] = json!(role);
        }
        Caller::read(&json!({"by": by, "owner": owner}))
    };
    assert_eq!(by(None, false), caller(40001, false));
    assert_eq!(by(Some("member"), false), caller(40001, false));
    assert_eq!(by(Some("manager"), false), caller(40001, true));
    assert_eq!(by(None, true), caller(40001, true));
    let nobody = Caller::read(&json!({"owner": false}));
    assert_eq!(
        nobody,
        Caller {
            id: None,
            manager: false
        }
    );
    // 私聊里终端管理员记成本人：照经的平台身份认。
    let admin = Caller::read(
        &json!({"by": {"kind": "person", "account": "admin", "via": "qq:10001"}, "owner": true}),
    );
    assert_eq!(admin, caller(10001, true));
    let local = Caller::read(&json!({"by": {"kind": "person", "account": "admin"}, "owner": true}));
    assert_eq!(local.id, None, "本机的人没有平台身份");
    let module = Caller::read(&json!({"by": {"kind": "module", "id": "qq:10001"}}));
    assert_eq!(module.id, None, "别的种类不认");
}

#[test]
fn recall_takes_the_quote_her_own_for_anyone_others_for_managers() {
    let mine = origin(40001, quote("90001", true, None), &[]);
    assert_eq!(
        in_group(Order::Recall, &caller(40001, false), Some(&mine)),
        Plan::Recall("90001".to_string())
    );
    let others = origin(40001, quote("101", false, Some(40002)), &[]);
    assert_eq!(
        in_group(Order::Recall, &caller(40001, false), Some(&others)),
        Plan::Refuse(Why::NotAllowed)
    );
    let others = origin(40003, quote("101", false, Some(40002)), &[]);
    assert_eq!(
        in_group(Order::Recall, &caller(40003, true), Some(&others)),
        Plan::Recall("101".to_string())
    );
    // 引用的那一条桥不认识：当别人的。
    let unknown = origin(40001, quote("4040", false, None), &[]);
    assert_eq!(
        in_group(Order::Recall, &caller(40001, false), Some(&unknown)),
        Plan::Refuse(Why::NotAllowed)
    );
    let bare = origin(40003, None, &[40002]);
    assert_eq!(
        in_group(Order::Recall, &caller(40003, true), Some(&bare)),
        Plan::Refuse(Why::NoQuote)
    );
    assert_eq!(
        in_group(Order::Recall, &caller(40003, true), None),
        Plan::Refuse(Why::NoQuote)
    );
}

#[test]
fn the_origin_must_come_from_the_caller() {
    // 叫她做的那条是别人的（和 `by` 对不上）：当没有。终端管理员的身份配不上别人的引用。
    let someone_elses = origin(40002, quote("101", false, Some(40001)), &[40001]);
    assert_eq!(
        in_group(Order::Recall, &caller(BOSS, true), Some(&someone_elses)),
        Plan::Refuse(Why::NoQuote)
    );
    assert_eq!(
        in_group(
            Order::Mute(Some(60)),
            &caller(BOSS, true),
            Some(&someone_elses)
        ),
        Plan::Refuse(Why::NoTarget)
    );
    assert_eq!(
        in_group(Order::Poke, &caller(BOSS, true), Some(&someone_elses)),
        Plan::Poke(qq(BOSS)),
        "戳叫她的人"
    );
    let unknown = Caller {
        id: None,
        manager: true,
    };
    assert_eq!(
        in_group(Order::Poke, &unknown, Some(&someone_elses)),
        Plan::Refuse(Why::NoTarget)
    );
}

#[test]
fn mute_takes_one_person_never_her_and_never_the_protected() {
    let boss = caller(BOSS, true);
    let mute = |origin: &Origin| in_group(Order::Mute(Some(60)), &boss, Some(origin));
    // 引用的和 @ 的是同一个人：一个。
    let same = origin(BOSS, quote("101", false, Some(40001)), &[40001]);
    assert_eq!(
        mute(&same),
        Plan::Mute {
            user: qq(40001),
            seconds: 60
        }
    );
    // 引用一个、@ 另一个：两个。
    let two = origin(BOSS, quote("101", false, Some(40001)), &[40002]);
    assert_eq!(mute(&two), Plan::Refuse(Why::ManyTargets));
    let two = origin(BOSS, None, &[40001, 40002]);
    assert_eq!(mute(&two), Plan::Refuse(Why::ManyTargets));
    // 引用的是她的、@ 的是她：都不算。
    let hers = origin(BOSS, quote("90001", true, None), &[]);
    assert_eq!(mute(&hers), Plan::Refuse(Why::NoTarget));
    let at_her = origin(BOSS, None, &[ME]);
    assert_eq!(mute(&at_her), Plan::Refuse(Why::NoTarget));
    let her_and_one = origin(BOSS, quote("90001", true, None), &[ME, 40001]);
    assert_eq!(
        mute(&her_and_one),
        Plan::Mute {
            user: qq(40001),
            seconds: 60
        }
    );
    // 引用的那一条桥不认识发的人：只看 @。
    let unknown = origin(BOSS, quote("7", false, None), &[]);
    assert_eq!(mute(&unknown), Plan::Refuse(Why::NoTarget));
    // 终端管理员、白名单成员动不得；解禁谁都解。
    let friend = origin(BOSS, None, &[FRIEND]);
    assert_eq!(mute(&friend), Plan::Refuse(Why::Protected(qq(FRIEND))));
    let manager = caller(40003, true);
    let at_boss = origin(40003, None, &[BOSS]);
    assert_eq!(
        in_group(Order::Mute(Some(60)), &manager, Some(&at_boss)),
        Plan::Refuse(Why::Protected(qq(BOSS)))
    );
    assert_eq!(
        in_group(Order::Mute(Some(0)), &boss, Some(&friend)),
        Plan::Mute {
            user: qq(FRIEND),
            seconds: 0
        }
    );
}

#[test]
fn only_managers_mute_and_the_caller_is_checked_before_the_seconds() {
    let at_one = origin(40001, None, &[40002]);
    assert_eq!(
        in_group(Order::Mute(Some(60)), &caller(40001, false), Some(&at_one)),
        Plan::Refuse(Why::NotAllowed)
    );
    assert_eq!(
        in_group(Order::Mute(None), &caller(40001, false), Some(&at_one)),
        Plan::Refuse(Why::NotAllowed)
    );
    let at_one = origin(40003, None, &[40002]);
    assert_eq!(
        in_group(Order::Mute(None), &caller(40003, true), Some(&at_one)),
        Plan::Refuse(Why::BadSeconds)
    );
}

#[test]
fn poke_takes_one_mention_or_the_caller() {
    let anyone = caller(40001, false);
    let at_one = origin(40001, None, &[ME, 40002]);
    assert_eq!(
        in_group(Order::Poke, &anyone, Some(&at_one)),
        Plan::Poke(qq(40002))
    );
    let at_two = origin(40001, None, &[40002, 40003]);
    assert_eq!(
        in_group(Order::Poke, &anyone, Some(&at_two)),
        Plan::Refuse(Why::ManyTargets)
    );
    let bare = origin(40001, quote("101", false, Some(40002)), &[]);
    assert_eq!(
        in_group(Order::Poke, &anyone, Some(&bare)),
        Plan::Poke(qq(40001)),
        "引用不算，戳叫她的人"
    );
}

#[test]
fn a_private_chat_recalls_any_quote_and_pokes_the_peer() {
    let me = qq(ME);
    let peer = qq(BOSS);
    let scene = Scene {
        peer: Some(&peer),
        me: &me,
        whitelist: &[],
        admin: &|_: &ExternalId| false,
    };
    let anyone = caller(BOSS, false);
    let quoted = origin(BOSS, quote("77", false, None), &[40002]);
    assert_eq!(
        plan(Order::Recall, &anyone, Some(&quoted), &scene),
        Plan::Recall("77".to_string()),
        "私聊不分谁的，交给 QQ"
    );
    assert_eq!(
        plan(Order::Poke, &anyone, Some(&quoted), &scene),
        Plan::Poke(peer.clone()),
        "私聊只戳对方"
    );
    assert_eq!(
        plan(
            Order::Recall,
            &anyone,
            Some(&origin(BOSS, None, &[])),
            &scene
        ),
        Plan::Refuse(Why::NoQuote)
    );
}

#[test]
fn every_refusal_names_its_sentence() {
    let names: Vec<&str> = [
        Why::NotAllowed,
        Why::NoQuote,
        Why::NoTarget,
        Why::ManyTargets,
        Why::Protected(qq(1)),
        Why::BadSeconds,
    ]
    .iter()
    .map(Why::name)
    .collect();
    assert_eq!(
        names,
        [
            "not-allowed",
            "no-quote",
            "no-target",
            "many-targets",
            "protected",
            "bad-seconds"
        ]
    );
}

#[test]
fn a_duration_is_written_in_human_units() {
    for (seconds, wanted) in [
        (1, "1s"),
        (45, "45s"),
        (60, "1m"),
        (600, "10m"),
        (3_600, "1h"),
        (5_400, "1h 30m"),
        (86_400, "1d"),
        (90_061, "1d 1h 1m 1s"),
        (2_592_000, "30d"),
    ] {
        assert_eq!(duration(seconds), wanted, "{seconds}");
    }
}
