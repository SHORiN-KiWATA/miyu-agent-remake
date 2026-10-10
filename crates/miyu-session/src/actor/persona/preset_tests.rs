//! 改了预设，下一个回合换上（施工 P-2 下）：工具面照新的预设重新筛，以前就有的照旧快照里的原样；记忆照开会话时的；装了没开
//! 的那一行跟着变；写错了的照旧；P-2（中）造的没有指纹的不换。

use super::test_support::{installed, look_now, names, setup, write_preset};
use super::*;
use miyu_policy::features::{Feature, Features};

const OFF: &str = "Installed but off in this session's preset:";

#[test]
fn a_changed_preset_refilters_the_face_and_keeps_old_entries() {
    let (_scratch, root, mut refresh) = setup("preset-face", "");
    assert!(matches!(look_now(&refresh), Seen::Same), "没改的不换");
    // 快照里那一件是以前的样子（程序升级过、描述改了）：换预设时照旧留着它。
    refresh.snapshot.tools[0].description = "Read, the old way.".to_string();
    let read = refresh.snapshot.tools[0].clone();
    write_preset(&root, "[tools]\nshell = false\n");
    let Seen::Swapped(snapshot, _, _, _) = look_now(&refresh) else {
        panic!("改了要换");
    };
    assert_eq!(names(&snapshot), ["read", "remember"]);
    assert_eq!(snapshot.tools[0], read, "留着的那一件原样");
    assert!(!snapshot.system.contains(OFF), "包都开着");
    refresh.snapshot = *snapshot;
    assert!(matches!(look_now(&refresh), Seen::Same), "换过以后不再换");
    write_preset(&root, "[software]\nbasesystem = false\n");
    let Seen::Swapped(snapshot, _, _, _) = look_now(&refresh) else {
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
    let Seen::Swapped(snapshot, _, _, _) = look_now(&refresh) else {
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
    let Seen::Swapped(snapshot, _, _, _) = look_now(&refresh) else {
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
    let Seen::Swapped(snapshot, _, _, _) = look_now(&refresh) else {
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
    let Seen::Swapped(snapshot, _, _, _) = look_now(&refresh) else {
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
        tooled: tooled(&refresh.tools.current(), Some(&installed())),
        roleplay: true,
        group: None,
        foreground: false,
    };
    refresh.snapshot = build(&refresh.resources, parts).expect("拼得成");
    assert_eq!(
        (&refresh.snapshot.persona, &refresh.snapshot.persona_digest),
        (&None, &None)
    );
    assert!(matches!(look_now(&refresh), Seen::Same), "没改的不换");
    write_preset(&root, "[tools]\nshell = false\n");
    let Seen::Swapped(snapshot, _, _, _) = look_now(&refresh) else {
        panic!("无人格的照样换预设");
    };
    assert_eq!(snapshot.persona, None);
    assert!(
        !snapshot.system.contains("You are Miyu."),
        "{}",
        snapshot.system
    );
}

/// 升级以前造的会话改了预设（施工 P-1 三补）：工具面没变也照样换，换出来是新的核心的字。
#[test]
fn an_upgraded_session_still_follows_its_preset() {
    let (_scratch, root, mut refresh) = setup("preset-upgraded", "");
    refresh.snapshot.core.facts.env = "<env/>\n".to_string();
    assert!(matches!(look_now(&refresh), Seen::Same), "光是升级不换");
    write_preset(&root, "[software]\nroleplay = false\n");
    let Seen::Swapped(snapshot, _, _, _) = look_now(&refresh) else {
        panic!("改了预设要换");
    };
    assert_eq!(
        names(&snapshot),
        ["read", "remember", "shell"],
        "工具面没变"
    );
    assert_ne!(snapshot.core.facts.env, "<env/>\n", "新的核心的字一起换上");
}

/// 以前的快照记的没开的是包的编号（施工 F-3 上）：升级以后照现在的功能读一样的算没改，不换快照、不断缓存；真改了的照换。
#[test]
fn an_old_pin_written_with_package_ids_is_not_a_change() {
    let (_scratch, root, mut refresh) = setup("preset-legacy", "[software]\nonebot = false\n");
    let feature = |id: &str, package: &str| Feature {
        id: id.to_string(),
        package: package.to_string(),
        tools: Vec::new(),
    };
    let features = Features::new(vec![
        feature("basesystem", "basesystem"),
        feature("memory", "memory"),
        feature("roleplay", "roleplay"),
        feature("qq", "onebot"),
    ]);
    refresh.presets.as_mut().expect("有预设的几层").features = features;
    let pin = refresh.snapshot.preset.as_mut().expect("有预设");
    pin.off = vec!["onebot".to_string()];
    assert!(
        matches!(look_now(&refresh), Seen::Same),
        "以前记的 onebot 就是现在的 qq"
    );
    write_preset(&root, "[software]\nonebot = false\nmemory = false\n");
    assert!(
        matches!(look_now(&refresh), Seen::Swapped(..)),
        "真改了的照换"
    );
}

/// 预设照功能开关（施工 F-3 上）：`[features]` 关掉一个功能，它下面的工具都不给，装了没开的那一行写功能的编号。
#[test]
fn a_feature_switched_off_takes_its_tools_away() {
    let (_scratch, root, mut refresh) = setup("preset-feature", "");
    let feature = |id: &str, package: &str, tools: &[&str]| Feature {
        id: id.to_string(),
        package: package.to_string(),
        tools: tools.iter().map(ToString::to_string).collect(),
    };
    refresh.presets.as_mut().expect("有预设的几层").features = Features::new(vec![
        feature("files", "basesystem", &["read"]),
        feature("commands", "basesystem", &["shell"]),
        feature("memory", "memory", &[]),
        feature("roleplay", "roleplay", &[]),
    ]);
    write_preset(&root, "[features]\ncommands = false\n");
    let Seen::Swapped(snapshot, _, _, _) = look_now(&refresh) else {
        panic!("改了要换");
    };
    assert_eq!(names(&snapshot), ["read", "remember"]);
    assert!(
        snapshot.system.ends_with(&format!("{OFF} commands.")),
        "{}",
        snapshot.system
    );
}

/// 换快照时也照装没装（施工 F-3 上）：人格多了提醒短语，可人设防失忆提醒没装，换上的快照里没有提醒。
#[test]
fn a_swap_leaves_reminders_out_when_they_are_not_installed() {
    let (_scratch, root, mut refresh) = setup("preset-noreminder", "");
    let feature = |id: &str| Feature {
        id: id.to_string(),
        package: id.to_string(),
        tools: Vec::new(),
    };
    refresh.presets.as_mut().expect("有预设的几层").features =
        Features::new(vec![feature("basesystem"), feature("memory")]);
    super::test_support::write(&root, "reminders.md", "Stay soft.\n");
    let Seen::Swapped(snapshot, _, _, _) = look_now(&refresh) else {
        panic!("人格改了要换");
    };
    assert_eq!(snapshot.reminder, None, "没装人设防失忆提醒");
}

/// 换成关了后台运行的预设（施工 T-1 上）：快照记下 `foreground`，以前就有的 `shell` 照旧快照里的原样；改回来又不记。
#[test]
fn a_preset_turning_background_off_marks_the_snapshot() {
    let (_scratch, root, mut refresh) = setup("preset-background", "");
    assert!(!refresh.snapshot.foreground);
    let shell = refresh
        .snapshot
        .tools
        .iter()
        .find(|entry| entry.name == "shell")
        .cloned()
        .expect("有 shell");
    write_preset(&root, "[features]\nbackground = false\n");
    let Seen::Swapped(snapshot, _, _, _) = look_now(&refresh) else {
        panic!("改了要换");
    };
    assert!(snapshot.foreground, "记下关着");
    assert!(
        snapshot.tools.contains(&shell),
        "以前就有的照原样：{:?}",
        snapshot.tools
    );
    refresh.snapshot = *snapshot;
    write_preset(&root, "");
    let Seen::Swapped(snapshot, _, _, _) = look_now(&refresh) else {
        panic!("改回来也换");
    };
    assert!(!snapshot.foreground, "开着的不记");
}
