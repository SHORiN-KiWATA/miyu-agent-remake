//! 改了预设，下一个回合换上（施工 P-2 下）：工具面照新的预设重新筛，以前就有的照旧快照里的原样；记忆照开会话时的；装了没开
//! 的那一行跟着变；写错了的照旧；P-2（中）造的没有指纹的不换。

use std::sync::Arc;

use miyu_config::Values;
use miyu_kernel::id::{AccountId, VenueId};
use miyu_kernel::tool::Access;
use miyu_policy::memory::MemoryScope;
use miyu_policy::preset::{self, Chosen};
use miyu_store::presets::Presets;
use miyu_store::root::DataRoot;
use miyu_tool::Tool;
use miyu_tool::testkit::{Act, Fake};

use std::path::Path;

use super::test_support::{Scratch, scratch_root};
use super::*;

/// 基础系统两件、记忆一件。
fn catalog() -> Catalog {
    let fake = |name: &str| -> Arc<dyn Tool> { Fake::new(name, Access::Read, Act::Echo) };
    Catalog::in_packages([
        ("basesystem", vec![fake("read"), fake("shell")]),
        ("memory", vec![fake("remember")]),
    ])
    .expect("合写法")
}

const INSTALLED: [&str; 3] = ["basesystem", "memory", "roleplay"];

/// 一个会话：人格 Miyu，预设是家目录里的 `p.toml`（内容 `text`），照它筛好的工具面、算好的范围拼的快照。
fn setup(name: &str, text: &str) -> (Scratch, DataRoot, Refresh) {
    let (scratch, root) = scratch_root(name, "You are Miyu.\n");
    write_preset(&root, text);
    let alice = AccountId::parse("alice").expect("账号合写法");
    let resources = ResourceRoot::at(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"));
    let personas = Personas::new(&resources, &root, &alice);
    let presets = Presets::new(&resources, &root, &alice);
    let found = presets.find("p").expect("找得到预设");
    let chosen = Chosen::new(found.id, found.file, INSTALLED);
    let venue = VenueId::parse("local").expect("场所合写法");
    let scope = if chosen.file.opens(preset::MEMORY) {
        MemoryScope::Persona
    } else {
        MemoryScope::Off
    };
    let tools = catalog();
    let face = Agents::face(
        &tools,
        &venue,
        None,
        &Offers::of(&Values::default(), Vec::new()),
        true,
        scope,
        Some(&chosen.file),
    );
    let parts = Parts {
        name: Some("miyu".to_string()),
        texts: personas.find("miyu").expect("找得到 Miyu").texts,
        attended: true,
        face,
        memory: Some(scope.as_str().to_string()),
        child: false,
        preset: Some(chosen.pin()),
        tooled: tooled(&tools),
        group: None,
    };
    let snapshot = build(&resources, parts).expect("拼得成");
    let refresh = Refresh {
        personas,
        resources,
        blobs: Blobs::new(root.blobs(&alice)),
        snapshot,
        child: false,
        presets: Some(PresetPlaces {
            presets,
            installed: INSTALLED.iter().map(|one| one.to_string()).collect(),
        }),
        tools,
        venue,
        lineage: None,
    };
    (scratch, root, refresh)
}

fn write_preset(root: &DataRoot, text: &str) {
    let dir = root.path().join("home/alice/presets");
    std::fs::create_dir_all(&dir).expect("建得了预设目录");
    std::fs::write(dir.join("p.toml"), text).expect("写得进");
}

fn look_now(refresh: &Refresh) -> Seen {
    look(refresh, &Values::default())
}

fn names(snapshot: &Snapshot) -> Vec<&str> {
    snapshot
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect()
}

const OFF: &str = "Installed but off in this session's preset:";

#[test]
fn a_changed_preset_refilters_the_face_and_keeps_old_entries() {
    let (_scratch, root, mut refresh) = setup("preset-face", "");
    assert!(matches!(look_now(&refresh), Seen::Same), "没改的不换");
    // 快照里那一件是以前的样子（程序升级过、描述改了）：换预设时照旧留着它。
    refresh.snapshot.tools[0].description = "Read, the old way.".to_string();
    let read = refresh.snapshot.tools[0].clone();
    write_preset(&root, "[tools]\nshell = false\n");
    let Seen::Swapped(snapshot, _, _) = look_now(&refresh) else {
        panic!("改了要换");
    };
    assert_eq!(names(&snapshot), ["read", "remember"]);
    assert_eq!(snapshot.tools[0], read, "留着的那一件原样");
    assert!(!snapshot.system.contains(OFF), "包都开着");
    refresh.snapshot = *snapshot;
    assert!(matches!(look_now(&refresh), Seen::Same), "换过以后不再换");
    write_preset(&root, "[software]\nbasesystem = false\n");
    let Seen::Swapped(snapshot, _, _) = look_now(&refresh) else {
        panic!("改了要换");
    };
    assert_eq!(names(&snapshot), ["remember"]);
    assert!(
        snapshot.system.ends_with(&format!("{OFF} basesystem.")),
        "{}",
        snapshot.system
    );
    refresh.snapshot = *snapshot;
    write_preset(&root, "");
    let Seen::Swapped(snapshot, _, _) = look_now(&refresh) else {
        panic!("改回来也换");
    };
    assert_eq!(
        names(&snapshot),
        ["read", "remember", "shell"],
        "重新打开的照现在的目录拿"
    );
    assert!(!snapshot.system.contains(OFF));
}

#[test]
fn memory_stays_as_it_was_when_the_session_was_made() {
    let (_scratch, root, refresh) = setup("preset-memory", "");
    write_preset(
        &root,
        "[software]\nmemory = false\n\n[tools]\nshell = false\n",
    );
    let Seen::Swapped(snapshot, _, _) = look_now(&refresh) else {
        panic!("改了要换");
    };
    assert_eq!(
        names(&snapshot),
        ["read", "remember"],
        "记忆照开会话时的开着"
    );
    assert!(!snapshot.system.contains(OFF), "记忆也不进那一行");
    assert_eq!(snapshot.memory.as_deref(), Some("persona"));
    let (_scratch, root, refresh) = setup("preset-memory-off", "[software]\nmemory = false\n");
    assert_eq!(names(&refresh.snapshot), ["read", "shell"]);
    write_preset(&root, "");
    let Seen::Swapped(snapshot, _, _) = look_now(&refresh) else {
        panic!("改了要换");
    };
    assert_eq!(names(&snapshot), ["read", "shell"], "关着的也照旧关着");
    assert!(snapshot.system.ends_with(&format!("{OFF} memory.")));
}

#[test]
fn roleplay_turned_on_brings_the_reminder_back() {
    let (_scratch, root, refresh) = setup("preset-roleplay", "[software]\nroleplay = false\n");
    let prompts = root.path().join("home/alice/personas/miyu/prompts");
    std::fs::write(prompts.join("reminders.md"), "Stay soft.\n").expect("写得进");
    write_preset(&root, "");
    let Seen::Swapped(snapshot, _, _) = look_now(&refresh) else {
        panic!("人格、预设都改了，换一次");
    };
    assert!(snapshot.reminder.is_some(), "角色扮演打开了");
}

#[test]
fn a_broken_preset_and_an_older_pin_are_kept() {
    let (_scratch, root, mut refresh) = setup("preset-broken", "");
    write_preset(&root, "[tools]\nshell = true\n");
    assert!(matches!(look_now(&refresh), Seen::Unreadable(_)));
    write_preset(&root, "[tools]\nshell = false\n");
    if let Some(pin) = refresh.snapshot.preset.as_mut() {
        pin.digest = None;
    }
    assert!(
        matches!(look_now(&refresh), Seen::Same),
        "P-2（中）造的快照没有指纹，不换"
    );
}

/// 无人格的会话（施工 P-4 上）：快照里没有人格的指纹，回合开始照样看预设，改了照样换；换出来的照旧无人格。
#[test]
fn without_a_persona_a_changed_preset_still_swaps() {
    let (_scratch, root, mut refresh) = setup("preset-nobody", "");
    let parts = Parts {
        name: None,
        texts: miyu_policy::PersonaTexts::default(),
        attended: true,
        face: refresh.snapshot.tools.clone(),
        memory: Some("off".to_string()),
        child: false,
        preset: refresh.snapshot.preset.clone(),
        tooled: tooled(&refresh.tools),
        group: None,
    };
    refresh.snapshot = build(&refresh.resources, parts).expect("拼得成");
    assert_eq!(
        (&refresh.snapshot.persona, &refresh.snapshot.persona_digest),
        (&None, &None)
    );
    assert!(matches!(look_now(&refresh), Seen::Same), "没改的不换");
    write_preset(&root, "[tools]\nshell = false\n");
    let Seen::Swapped(snapshot, _, _) = look_now(&refresh) else {
        panic!("无人格的照样换预设");
    };
    assert_eq!(snapshot.persona, None);
    assert!(
        !snapshot.system.contains("You are Miyu."),
        "{}",
        snapshot.system
    );
}
