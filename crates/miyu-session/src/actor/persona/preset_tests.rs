//! 改了预设，下一个回合换上（施工 P-2 下）：工具面照新的预设重新筛，以前就有的照旧快照里的原样；记忆照开会话时的；装了没开
//! 的那一行跟着变；写错了的照旧；P-2（中）造的没有指纹的不换。

use super::test_support::{look_now, names, setup, write_preset};
use super::*;

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
        tooled: tooled(&refresh.tools.current()),
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
