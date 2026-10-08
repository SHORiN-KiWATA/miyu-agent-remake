//! 看一遍人格改了没有（施工 P-1 再补）：没改的不换；改了的换，新快照存进 blob；以前造的快照没有指纹的不换；程序升级过、
//! 核心的字和旧快照不一样的不换；人格写错了的照旧。人格放在临时数据根里管理员家目录那一层。

use std::path::Path;
use std::sync::Arc;

use miyu_config::Values;
use miyu_kernel::id::{AccountId, VenueId};
use miyu_kernel::tool::Access;
use miyu_store::root::DataRoot;
use miyu_tool::Tool;
use miyu_tool::testkit::{Act, Fake};

use super::test_support::{Scratch, scratch_root, write};
use super::*;

/// 照全是默认值的配置看一遍。
fn seen(refresh: &Refresh) -> Seen {
    look(refresh, &Values::default(), &refresh.tools.edition())
}

/// 一个临时数据根，Miyu 住在管理员 alice 的家目录，人设是 `persona`；和照这一刻的文件拼好快照的 `Refresh`。
fn setup(name: &str, persona: &str) -> (Scratch, DataRoot, Refresh) {
    setup_with(name, persona, None, None)
}

/// 同 [`setup`]，快照记着预设 `preset`（施工 P-2 中）；`group` 有的是群会话，时区是它（施工 O-13 中）。
fn setup_with(
    name: &str,
    persona: &str,
    preset: Option<miyu_policy::PresetPin>,
    group: Option<i32>,
) -> (Scratch, DataRoot, Refresh) {
    let (scratch, root) = scratch_root(name, persona);
    let alice = AccountId::parse("alice").expect("账号合写法");
    let resources = ResourceRoot::at(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"));
    let personas = Personas::new(&resources, &root, &alice);
    // 记忆有一件工具（装了没开的那一行只列有工具的包，施工 O-18）；工具面是空的。
    let remember: Arc<dyn Tool> = Fake::new("remember", Access::Read, Act::Echo);
    let tools = Catalog::in_packages([("memory", vec![remember])]).expect("合写法");
    let parts = Parts {
        name: Some("miyu".to_string()),
        texts: personas.find("miyu").expect("找得到 Miyu").texts,
        attended: true,
        face: Vec::new(),
        memory: None,
        child: false,
        preset,
        tooled: tooled(&tools),
        group,
    };
    let snapshot = build(&resources, parts).expect("拼得成");
    let refresh = Refresh {
        personas,
        resources,
        blobs: Blobs::new(root.blobs(&alice)),
        snapshot,
        child: false,
        presets: None,
        seen: Some(Shelf::new(tools.clone()).edition()),
        tools: Shelf::new(tools),
        venue: VenueId::parse("local").expect("场所合写法"),
        lineage: None,
    };
    (scratch, root, refresh)
}

#[test]
fn an_unchanged_persona_is_the_same_and_a_changed_one_swaps() {
    let (_scratch, root, refresh) = setup("swap", "You are Miyu.\n");
    assert!(matches!(seen(&refresh), Seen::Same));
    write(&root, "persona.md", "You are Miyu, softly.\n");
    let Seen::Swapped(snapshot, _, hash) = seen(&refresh) else {
        panic!("改了要换");
    };
    assert!(snapshot.system.starts_with("You are Miyu, softly."));
    assert_eq!(hash, snapshot.hash());
    assert_eq!(
        refresh.blobs.get(&hash).expect("新快照存进了 blob"),
        snapshot.to_bytes()
    );
}

/// 群会话换人格（施工 O-13 中）：新快照照样在人设后面接着格式说明，时区照旧快照钉下的。
#[test]
fn a_group_keeps_its_note_and_time_zone_across_a_swap() {
    let (_scratch, root, refresh) = setup_with("group-swap", "You are Miyu.\n", None, Some(-300));
    let note = refresh.resources.group_note().expect("读得到格式说明");
    assert!(
        refresh
            .snapshot
            .system
            .starts_with(&format!("You are Miyu.\n\n{}", note.trim_end()))
    );
    write(&root, "persona.md", "You are Miyu, softly.\n");
    let Seen::Swapped(snapshot, _, _) = seen(&refresh) else {
        panic!("改了要换");
    };
    assert!(
        snapshot
            .system
            .starts_with(&format!("You are Miyu, softly.\n\n{}", note.trim_end())),
        "{}",
        snapshot.system
    );
    assert_eq!(snapshot.group.as_ref().map(|chat| chat.offset), Some(-300));
}

#[test]
fn an_older_snapshot_without_a_digest_is_never_swapped() {
    let (_scratch, root, mut refresh) = setup("older", "You are Miyu.\n");
    refresh.snapshot.persona_digest = None;
    write(&root, "persona.md", "You are Miyu, softly.\n");
    assert!(matches!(seen(&refresh), Seen::Same));
}

#[test]
fn a_session_made_before_an_upgrade_keeps_its_snapshot() {
    let (_scratch, root, mut refresh) = setup("upgraded", "You are Miyu.\n");
    refresh.snapshot.core.facts.env = "<env/>\n".to_string();
    write(&root, "persona.md", "You are Miyu, softly.\n");
    assert!(matches!(seen(&refresh), Seen::Kept(_)));
}

#[test]
fn a_broken_persona_is_unreadable_and_kept() {
    let (_scratch, root, refresh) = setup("broken", "You are Miyu.\n");
    write(&root, "examples.md", "user: a\n");
    assert!(matches!(seen(&refresh), Seen::Unreadable(_)));
}

/// 预设没开角色扮演的（施工 P-2 中）：换人格时照旧没有角色扮演提示和风格锁；指纹照人格原来的字算，换过以后不会每轮都当成
/// 改过。
#[test]
fn roleplay_stays_off_across_a_swap() {
    let pin = miyu_policy::PresetPin {
        id: "dev".to_string(),
        off: vec!["memory".to_string(), "roleplay".to_string()],
        digest: None,
    };
    let (_scratch, root, mut refresh) =
        setup_with("roleplay", "You are Miyu.\n", Some(pin.clone()), None);
    write(&root, "reminders.md", "Stay soft.\n");
    let Seen::Swapped(snapshot, _, _) = seen(&refresh) else {
        panic!("多了角色扮演提示也算改了");
    };
    assert_eq!(snapshot.reminder, None, "预设没开角色扮演");
    assert!(
        snapshot
            .system
            .ends_with("Installed but off in this session's preset: memory."),
        "没开的那一行照旧在最后，没有风格锁：{}",
        snapshot.system
    );
    assert_eq!(snapshot.preset, Some(pin));
    refresh.snapshot = *snapshot;
    assert!(matches!(seen(&refresh), Seen::Same), "换过以后不再换");
}

/// 人格的文件没了（施工 P-4 上：以前钉着出厂人格、后来撤掉了的会话）：照快照里的接着用，不换也不报。
#[test]
fn a_persona_whose_files_are_gone_is_kept_quietly() {
    let (_scratch, root, refresh) = setup("gone", "You are Miyu.\n");
    std::fs::remove_dir_all(root.path().join("home/alice/personas/miyu")).expect("删得掉");
    assert!(matches!(seen(&refresh), Seen::Same));
}
