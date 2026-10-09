//! 包的工具变了，下一个回合换上（施工 O-2 中）：目录换了代，提供者的工具照现在的登记加上、改掉、去掉；自带的照旧快照里的
//! 原样、不新加，改过名的照旧叫以前的名字；只给群的不进本机的会话，拼出来一样的不换；载入的不知道是哪一代，第一个回合对
//! 一次；以前造的预设没有指纹的不跟。

use std::sync::Arc;

use miyu_kernel::raw::RawJson;
use miyu_kernel::tool::Access;
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Call, Progress, Running, Spec, Tool, Venues};

use super::test_support::{look_now, names, setup};
use super::*;

const LOCAL: Venues = Venues {
    local: true,
    private: false,
    group: false,
};

const GROUP: Venues = Venues {
    local: false,
    private: false,
    group: true,
};

fn provided(name: &str, venues: Venues) -> Arc<dyn Tool> {
    Fake::in_venues(name, Access::Read, venues, Act::Echo)
}

/// 桥照 `tools` 登记一次。
fn provide(refresh: &Refresh, tools: Vec<Arc<dyn Tool>>) {
    refresh
        .tools
        .replace(|catalog| catalog.replacing("onebot", tools))
        .expect("登记得上");
}

/// 看一遍，该换：照新的快照，记下现在这一代。
fn swap(refresh: &mut Refresh) -> Snapshot {
    let Seen::Swapped(snapshot, _, _, _) = look_now(refresh) else {
        panic!("该换");
    };
    refresh.snapshot = (*snapshot).clone();
    refresh.seen = Some(refresh.tools.edition());
    *snapshot
}

#[test]
fn provided_tools_join_change_and_leave_the_face() {
    let (_scratch, _root, mut refresh) = setup("shelf-join", "");
    provide(&refresh, vec![provided("send", LOCAL)]);
    let snapshot = swap(&mut refresh);
    assert_eq!(names(&snapshot), ["read", "remember", "send", "shell"]);
    assert!(matches!(look_now(&refresh), Seen::Same), "换过以后不再换");
    let changed = Spec {
        name: "send".to_string(),
        description: "Send, the new way.".to_string(),
        parameters: serde_json::from_str::<RawJson>(r#"{"type":"object"}"#).unwrap(),
        access: Access::Read,
    };
    provide(&refresh, vec![Fake::from_spec(changed, Act::Echo)]);
    let snapshot = swap(&mut refresh);
    let send = snapshot.tools.iter().find(|tool| tool.name == "send");
    assert_eq!(
        send.map(|tool| tool.description.as_str()),
        Some("Send, the new way."),
        "改了说明的照现在的"
    );
    provide(&refresh, Vec::new());
    let snapshot = swap(&mut refresh);
    assert_eq!(
        names(&snapshot),
        ["read", "remember", "shell"],
        "去掉了的拿掉"
    );
}

#[test]
fn built_in_tools_stay_as_they_were() {
    let (_scratch, _root, mut refresh) = setup("shelf-kept", "");
    // 快照里那一件是以前的样子（程序升级过、描述改了）：目录换代时照旧留着它。
    refresh.snapshot.tools[0].description = "Read, the old way.".to_string();
    let read = refresh.snapshot.tools[0].clone();
    let fake = |name: &str| -> Arc<dyn Tool> { Fake::new(name, Access::Read, Act::Echo) };
    refresh
        .tools
        .replace(|_| {
            Catalog::in_packages([
                (
                    "basesystem",
                    vec![fake("glob"), fake("read"), fake("shell")],
                ),
                ("memory", vec![fake("remember")]),
            ])?
            .replacing("onebot", vec![provided("send", LOCAL)])
        })
        .expect("登记得上");
    let snapshot = swap(&mut refresh);
    assert_eq!(
        names(&snapshot),
        ["read", "remember", "send", "shell"],
        "新的自带工具等预设改了才进来"
    );
    assert_eq!(snapshot.tools[0], read, "留着的那一件原样");
}

#[test]
fn a_tool_for_groups_only_leaves_a_local_session_alone() {
    let (_scratch, _root, refresh) = setup("shelf-group", "");
    provide(&refresh, vec![provided("mute", GROUP)]);
    assert!(matches!(look_now(&refresh), Seen::Same), "拼出来一样的不换");
}

#[test]
fn a_loaded_session_checks_the_catalog_once() {
    let (_scratch, _root, mut refresh) = setup("shelf-loaded", "");
    refresh.seen = None;
    assert!(matches!(look_now(&refresh), Seen::Same), "目录和快照对得上");
    // 程序升级拿掉了一件自带的：载入的认不出它是哪来的，照旧留着，调到时暂时不可用（施工 4-2）。
    let fake = |name: &str| -> Arc<dyn Tool> { Fake::new(name, Access::Read, Act::Echo) };
    refresh
        .tools
        .replace(|_| {
            Catalog::in_packages([
                ("basesystem", vec![fake("read")]),
                ("memory", vec![fake("remember")]),
            ])
        })
        .expect("登记得上");
    assert!(
        matches!(look_now(&refresh), Seen::Same),
        "目录里没有了的自带工具照旧留着"
    );
    provide(&refresh, vec![provided("send", LOCAL)]);
    refresh.seen = Some(refresh.tools.edition());
    assert!(
        matches!(look_now(&refresh), Seen::Same),
        "认得是哪一代、没换代的不看"
    );
    refresh.seen = None;
    let snapshot = swap(&mut refresh);
    assert_eq!(names(&snapshot), ["read", "remember", "send", "shell"]);
}

#[test]
fn an_older_pin_does_not_follow_the_catalog() {
    let (_scratch, _root, mut refresh) = setup("shelf-older", "");
    if let Some(pin) = refresh.snapshot.preset.as_mut() {
        pin.digest = None;
    }
    provide(&refresh, vec![provided("send", LOCAL)]);
    assert!(
        matches!(look_now(&refresh), Seen::Same),
        "P-2（中）造的快照找不回预设，不跟"
    );
}

/// 改过名的一件（施工 7-5 再补）：规格、执行照 `tool`，以前叫 `formerly`。
struct Former(Arc<dyn Tool>, &'static [&'static str]);

impl Tool for Former {
    fn spec(&self) -> &Spec {
        self.0.spec()
    }

    fn run(&self, call: Call, progress: Progress) -> Running<'_> {
        self.0.run(call, progress)
    }

    fn formerly(&self) -> &'static [&'static str] {
        self.1
    }
}

#[test]
fn a_renamed_tool_keeps_its_old_name() {
    let (_scratch, _root, mut refresh) = setup("shelf-renamed", "");
    let fake = |name: &str| -> Arc<dyn Tool> { Fake::new(name, Access::Read, Act::Echo) };
    let renamed: Arc<dyn Tool> = Arc::new(Former(fake("sh"), &["shell"]));
    refresh
        .tools
        .replace(|_| {
            Catalog::in_packages([
                ("basesystem", vec![fake("read"), renamed]),
                ("memory", vec![fake("remember")]),
            ])?
            .replacing("onebot", vec![provided("send", LOCAL)])
        })
        .expect("登记得上");
    let snapshot = swap(&mut refresh);
    assert_eq!(
        names(&snapshot),
        ["read", "remember", "send", "shell"],
        "快照里冻着旧名字"
    );
}

/// 没有预设的会话（P-2 以前造的）照样跟着目录。
#[test]
fn a_session_without_a_preset_follows_the_catalog_too() {
    let (_scratch, _root, mut refresh) = setup("shelf-nopreset", "");
    let parts = Parts {
        name: refresh.snapshot.persona.clone(),
        texts: refresh.personas.find("miyu").expect("找得到 Miyu").texts,
        attended: true,
        face: refresh.snapshot.tools.clone(),
        memory: refresh.snapshot.memory.clone(),
        child: false,
        preset: None,
        tooled: tooled(&refresh.tools.current()),
        group: None,
    };
    refresh.snapshot = build(&refresh.resources, parts).expect("拼得成");
    assert!(matches!(look_now(&refresh), Seen::Same), "没换代的不换");
    provide(&refresh, vec![provided("send", LOCAL)]);
    let snapshot = swap(&mut refresh);
    assert_eq!(names(&snapshot), ["read", "remember", "send", "shell"]);
}

/// 关着的包登记了工具：工具面里没有，装了没开的那一行带上它。找不回预设的快照这一行也不跟。
#[test]
fn a_provided_package_that_is_off_joins_the_off_line() {
    let act = || provided("act", LOCAL);
    let (_scratch, _root, mut refresh) = setup("shelf-off-line", "[software]\nroleplay = false\n");
    refresh
        .tools
        .replace(|catalog| catalog.replacing("roleplay", vec![act()]))
        .expect("登记得上");
    let snapshot = swap(&mut refresh);
    assert_eq!(names(&snapshot), ["read", "remember", "shell"]);
    assert!(
        snapshot
            .system
            .ends_with("Installed but off in this session's preset: roleplay."),
        "{}",
        snapshot.system
    );
    let (_scratch, _root, mut older) = setup("shelf-older-line", "[software]\nroleplay = false\n");
    if let Some(pin) = older.snapshot.preset.as_mut() {
        pin.digest = None;
    }
    older
        .tools
        .replace(|catalog| catalog.replacing("roleplay", vec![act()]))
        .expect("登记得上");
    assert!(matches!(look_now(&older), Seen::Same), "找不回预设的不跟");
}

/// 预设改了、同时提供者改了说明：自带的照旧快照里的，提供者的照现在的登记。
#[test]
fn a_changed_preset_takes_provided_tools_as_registered_now() {
    let (_scratch, root, mut refresh) = setup("shelf-preset", "");
    provide(&refresh, vec![provided("send", LOCAL)]);
    swap(&mut refresh);
    let changed = Spec {
        name: "send".to_string(),
        description: "Send, the new way.".to_string(),
        parameters: serde_json::from_str::<RawJson>(r#"{"type":"object"}"#).unwrap(),
        access: Access::Read,
    };
    provide(&refresh, vec![Fake::from_spec(changed, Act::Echo)]);
    super::test_support::write_preset(&root, "[tools]\nshell = false\n");
    let snapshot = swap(&mut refresh);
    assert_eq!(names(&snapshot), ["read", "remember", "send"]);
    let send = snapshot.tools.iter().find(|tool| tool.name == "send");
    assert_eq!(
        send.map(|tool| tool.description.as_str()),
        Some("Send, the new way.")
    );
}

/// 升级以前造的会话、只是目录换了代（施工 P-1 三补）：工具面真的变了的换，换出来是新的核心的字；没变的（只给群的工具、
/// 关着的包进了装了没开的那一行）不换，不为升级断一次缓存。
#[test]
fn after_an_upgrade_only_a_changed_face_swaps() {
    let older = |name: &str, text: &str| {
        let (scratch, root, mut refresh) = setup(name, text);
        refresh.snapshot.core.facts.env = "<env/>\n".to_string();
        (scratch, root, refresh)
    };
    let (_scratch, _root, mut refresh) = older("upgrade-face", "");
    provide(&refresh, vec![provided("send", LOCAL)]);
    let snapshot = swap(&mut refresh);
    assert_eq!(names(&snapshot), ["read", "remember", "send", "shell"]);
    assert_ne!(snapshot.core.facts.env, "<env/>\n", "新的核心的字一起换上");
    let (_scratch, _root, refresh) = older("upgrade-group", "");
    provide(&refresh, vec![provided("mute", GROUP)]);
    assert!(matches!(look_now(&refresh), Seen::Same), "工具面没变的不换");
    let (_scratch, _root, refresh) = older("upgrade-line", "[software]\nroleplay = false\n");
    refresh
        .tools
        .replace(|catalog| catalog.replacing("roleplay", vec![provided("act", LOCAL)]))
        .expect("登记得上");
    assert!(
        matches!(look_now(&refresh), Seen::Same),
        "只有装了没开的那一行变了的不换"
    );
    let (_scratch, _root, mut refresh) = older("upgrade-loaded", "");
    refresh.seen = None;
    assert!(
        matches!(look_now(&refresh), Seen::Same),
        "载入时对一次：光是升级不换"
    );
}
