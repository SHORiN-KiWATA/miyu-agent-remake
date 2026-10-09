//! 换快照的测试共用的（施工 P-2 下从 `tests.rs` 挪出来）：一个临时数据根，Miyu 住在管理员 alice 的家目录；有预设的会话
//! （施工 O-2 中从 `preset_tests.rs` 挪出来，目录换代的测试也用）。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use miyu_config::Values;
use miyu_kernel::id::{AccountId, VenueId};
use miyu_kernel::tool::Access;
use miyu_policy::memory::MemoryScope;
use miyu_policy::preset::{self, Chosen};
use miyu_store::env::{Env, Platform};
use miyu_store::presets::Presets;
use miyu_store::root::DataRoot;
use miyu_tool::Tool;
use miyu_tool::testkit::{Act, Fake};

use super::*;

/// 临时目录：用完删掉。
pub(super) struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        drop(std::fs::remove_dir_all(&self.0));
    }
}

/// 一个临时数据根，Miyu 的人设是 `persona`。
pub(super) fn scratch_root(name: &str, persona: &str) -> (Scratch, DataRoot) {
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
    (scratch, root)
}

/// 写 Miyu 的一份字。
pub(super) fn write(root: &DataRoot, file: &str, text: &str) {
    let dir = root.path().join("home/alice/personas/miyu/prompts");
    std::fs::create_dir_all(&dir).expect("建得了人格目录");
    std::fs::write(dir.join(file), text).expect("写得进");
}

/// 基础系统两件、记忆一件。
pub(super) fn catalog() -> Catalog {
    let fake = |name: &str| -> Arc<dyn Tool> { Fake::new(name, Access::Read, Act::Echo) };
    Catalog::in_packages([
        ("basesystem", vec![fake("read"), fake("shell")]),
        ("memory", vec![fake("remember")]),
    ])
    .expect("合写法")
}

pub(super) const INSTALLED: [&str; 3] = ["basesystem", "memory", "roleplay"];

/// 一个会话：人格 Miyu，预设是家目录里的 `p.toml`（内容 `text`），照它筛好的工具面、算好的范围拼的快照。
pub(super) fn setup(name: &str, text: &str) -> (Scratch, DataRoot, Refresh) {
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
        crate::agents::Site {
            venue: &venue,
            group: false,
        },
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
        seen: Some(Shelf::new(tools.clone()).edition()),
        tools: Shelf::new(tools),
        venue,
        lineage: None,
    };
    (scratch, root, refresh)
}

pub(super) fn write_preset(root: &DataRoot, text: &str) {
    let dir = root.path().join("home/alice/presets");
    std::fs::create_dir_all(&dir).expect("建得了预设目录");
    std::fs::write(dir.join("p.toml"), text).expect("写得进");
}

pub(super) fn look_now(refresh: &Refresh) -> Seen {
    look(refresh, &Values::default(), &refresh.tools.edition())
}

pub(super) fn names(snapshot: &Snapshot) -> Vec<&str> {
    snapshot
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect()
}
