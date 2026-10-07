//! 看一遍人格改了没有（施工 P-1 再补）：没改的不换；改了的换，新快照存进 blob；以前造的快照没有指纹的不换；程序升级过、
//! 核心的字和旧快照不一样的不换；人格写错了的照旧。人格放在临时数据根里管理员家目录那一层。

use std::path::{Path, PathBuf};

use miyu_kernel::id::AccountId;
use miyu_store::env::{Env, Platform};
use miyu_store::root::DataRoot;

use super::*;

/// 临时目录：用完删掉。
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        drop(std::fs::remove_dir_all(&self.0));
    }
}

/// 一个临时数据根，Miyu 住在管理员 alice 的家目录，人设是 `persona`；和照这一刻的文件拼好快照的 `Refresh`。
fn setup(name: &str, persona: &str) -> (Scratch, DataRoot, Refresh) {
    let scratch =
        Scratch(std::env::temp_dir().join(format!("miyu-persona-{name}-{}", std::process::id())));
    let env = Env {
        platform: Platform::current(),
        miyu_home: Some(scratch.0.join("data").into_os_string()),
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    };
    let root = DataRoot::locate(&env).expect("MIYU_HOME 是绝对路径");
    root.prepare().expect("临时目录里建得了骨架");
    write(&root, "persona.md", persona);
    let alice = AccountId::parse("alice").expect("账号合写法");
    let resources = ResourceRoot::at(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"));
    let personas = Personas::new(&resources, &root, &alice);
    let parts = Parts {
        name: "miyu".to_string(),
        texts: personas.find("miyu").expect("找得到 Miyu").texts,
        attended: true,
        face: Vec::new(),
        memory: None,
        child: false,
    };
    let snapshot = build(&resources, parts).expect("拼得成");
    let refresh = Refresh {
        personas,
        resources,
        blobs: Blobs::new(root.blobs(&alice)),
        snapshot,
        child: false,
    };
    (scratch, root, refresh)
}

/// 写 Miyu 的一份字。
fn write(root: &DataRoot, file: &str, text: &str) {
    let dir = root.path().join("home/alice/personas/miyu/prompts");
    std::fs::create_dir_all(&dir).expect("建得了人格目录");
    std::fs::write(dir.join(file), text).expect("写得进");
}

#[test]
fn an_unchanged_persona_is_the_same_and_a_changed_one_swaps() {
    let (_scratch, root, refresh) = setup("swap", "You are Miyu.\n");
    assert!(matches!(look(&refresh), Seen::Same));
    write(&root, "persona.md", "You are Miyu, softly.\n");
    let Seen::Swapped(snapshot, _, hash) = look(&refresh) else {
        panic!("改了要换");
    };
    assert!(snapshot.system.starts_with("You are Miyu, softly."));
    assert_eq!(hash, snapshot.hash());
    assert_eq!(
        refresh.blobs.get(&hash).expect("新快照存进了 blob"),
        snapshot.to_bytes()
    );
}

#[test]
fn an_older_snapshot_without_a_digest_is_never_swapped() {
    let (_scratch, root, mut refresh) = setup("older", "You are Miyu.\n");
    refresh.snapshot.persona_digest = None;
    write(&root, "persona.md", "You are Miyu, softly.\n");
    assert!(matches!(look(&refresh), Seen::Same));
}

#[test]
fn a_session_made_before_an_upgrade_keeps_its_snapshot() {
    let (_scratch, root, mut refresh) = setup("upgraded", "You are Miyu.\n");
    refresh.snapshot.core.facts.env = "<env/>\n".to_string();
    write(&root, "persona.md", "You are Miyu, softly.\n");
    assert!(matches!(look(&refresh), Seen::Kept(_)));
}

#[test]
fn a_broken_persona_is_unreadable_and_kept() {
    let (_scratch, root, refresh) = setup("broken", "You are Miyu.\n");
    write(&root, "examples.md", "user: a\n");
    assert!(matches!(look(&refresh), Seen::Unreadable(_)));
}
