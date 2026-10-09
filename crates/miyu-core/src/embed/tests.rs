//! 照包拼 `Embedder` 要的（施工 R-5 三补）：人格记忆装着、推荐了 `embed`、`embed` 是装着的小程序包，才拼；程序照小程序清单在
//! 主程序旁边找，模型清单、模型目录在包目录里。哪一样不对都没有。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use miyu_store::env::{Env, Platform};
use miyu_store::packages::{Found, Layer};

use super::*;

const MEMORY: &str = "[package]\nkind = \"builtin\"\nprotocol = [1, 1]\nname = { en = \"Persona memory\" }\n\n[recommends]\nworkers = [\"embed\"]\n";
const PLAIN_MEMORY: &str =
    "[package]\nkind = \"builtin\"\nprotocol = [1, 1]\nname = { en = \"Persona memory\" }\n";
const EMBED: &str = "[package]\nkind = \"worker\"\nprotocol = [1, 1]\nname = { en = \"Built-in semantic model\" }\n\n[worker]\nprogram = \"miyu-embed\"\n";
const NOT_A_WORKER: &str =
    "[package]\nkind = \"ui\"\nprotocol = [1, 1]\nname = { en = \"Not a worker\" }\n";

fn found(dir: &Path, id: &str, text: &str) -> Found {
    Found {
        id: id.to_string(),
        layer: Layer::Home,
        path: dir.join(format!("{id}.toml")),
        read: miyu_config::package::read(text).map_err(miyu_store::packages::Issue::Wrong),
    }
}

/// 一个假的主程序：`bin/miyu`，旁边放一个 `miyu-embed`。
fn env_with_program(root: &Path) -> Env {
    let bin = root.join("bin");
    std::fs::create_dir_all(&bin).expect("建得了");
    let exe = bin.join(format!("miyu{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&exe, b"").expect("写得进");
    std::fs::write(
        bin.join(format!("miyu-embed{}", std::env::consts::EXE_SUFFIX)),
        b"",
    )
    .expect("写得进");
    Env {
        platform: Platform::current(),
        miyu_home: None,
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: Some(exe),
    }
}

/// 一个用完就删的临时目录（照 `settings/tests.rs` 的）。
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("miyu-core-embed-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("建得了");
        Scratch(dir)
    }
}

impl Drop for Scratch {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn an_installed_embed_worker_recommended_by_memory_is_set_up_from_its_package() {
    let scratch = Scratch::new();
    let root = &scratch.0;
    let packages = root.join("packages");
    let env = env_with_program(root);
    let setup = setup(
        &env,
        &[
            found(&packages, "memory", MEMORY),
            found(&packages, "embed", EMBED),
        ],
    )
    .expect("拼得出");
    assert_eq!(setup.dir, packages.join("embed"));
    assert_eq!(setup.manifest, packages.join("embed").join("model.toml"));
    let program = setup.program.expect("在主程序旁边");
    assert_eq!(
        program.file_stem().and_then(|stem| stem.to_str()),
        Some("miyu-embed")
    );
    assert_eq!(setup.idle, EMBED_IDLE);
}

#[test]
fn anything_missing_gives_no_setup() {
    let scratch = Scratch::new();
    let root = &scratch.0;
    let packages = root.join("packages");
    let env = env_with_program(root);
    for (what, list) in [
        ("没装 embed", vec![found(&packages, "memory", MEMORY)]),
        ("人格记忆没装", vec![found(&packages, "embed", EMBED)]),
        (
            "人格记忆没推荐",
            vec![
                found(&packages, "memory", PLAIN_MEMORY),
                found(&packages, "embed", EMBED),
            ],
        ),
        (
            "embed 不是小程序",
            vec![
                found(&packages, "memory", MEMORY),
                found(&packages, "embed", NOT_A_WORKER),
            ],
        ),
        (
            "embed 写坏了",
            vec![
                found(&packages, "memory", MEMORY),
                found(&packages, "embed", "[package]\n"),
            ],
        ),
    ] {
        assert!(setup(&env, &list).is_none(), "{what}");
    }
}

#[test]
fn without_the_program_next_to_miyu_the_setup_says_so() {
    let scratch = Scratch::new();
    let root = &scratch.0;
    let packages = root.join("packages");
    let mut env = env_with_program(root);
    env.exe = Some(root.join("elsewhere").join("miyu"));
    let setup = setup(
        &env,
        &[
            found(&packages, "memory", MEMORY),
            found(&packages, "embed", EMBED),
        ],
    )
    .expect("包装着就拼");
    assert_eq!(setup.program, None, "程序不在：Embedder 造的时候报用不了");
}

/// 装卸时照清单拼（施工 F-5 再补，[`find`]）：和起来时拼的一样，说得出缺的是哪一样。「没装」那一行只有起来时那一路记：
/// 装订阅者的测试另在一个程序里（`tests/embed_log.rs`）。
#[test]
fn find_matches_setup_and_says_what_is_missing() {
    let scratch = Scratch::new();
    let root = &scratch.0;
    let packages = root.join("packages");
    let env = env_with_program(root);
    let memory = found(&packages, "memory", MEMORY);
    let plain = found(&packages, "memory", PLAIN_MEMORY);
    let embed = found(&packages, "embed", EMBED);
    let started = setup(
        &env,
        &[
            found(&packages, "memory", MEMORY),
            found(&packages, "embed", EMBED),
        ],
    );
    assert_eq!(find(&env, &[&memory, &embed]).ok(), started);
    assert!(started.is_some());
    assert_eq!(find(&env, &[&memory]), Err(Missing::Package));
    assert_eq!(find(&env, &[&plain, &embed]), Err(Missing::Wanted));
    assert_eq!(find(&env, &[&embed]), Err(Missing::Wanted));
}
