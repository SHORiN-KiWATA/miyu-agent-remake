//! 升级以后老会话照样换快照（施工 P-1 三补，`docs/construction/P-1-人格（三补）.md`）：会话开着的时候程序换了核心的字（这里
//! 改资源目录里的一份），光是这样不换；再改人格，下一个回合换上，执行器手里的字跟着换：权限策略拒绝时说的是新的那一句，请求
//! 模型的端口收到新的驱动占位。

use std::path::Path;
use std::sync::Arc;

use serde_json::json;

use miyu_endpoint::Core;
use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Body;
use miyu_session::testkit::{Play, Script};
use miyu_store::resources::ResourceRoot;
use miyu_tool::Catalog;

use crate::support::*;

/// 把 `from` 整个拷到 `to`。
fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("建得了目录");
    for entry in std::fs::read_dir(from).expect("读得了目录") {
        let entry = entry.expect("读得了目录项");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("认得出种类").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("拷得了");
        }
    }
}

/// 一份核心：资源照 `resources`，有基础系统的工具，人格、预设照配置找。
fn core_at(home: &Home, script: &Script, resources: &Path) -> Arc<Core> {
    let items = [
        miyu_endpoint::settings::UiSettings::ITEMS,
        miyu_endpoint::settings::PersonaSettings::ITEMS,
        miyu_endpoint::settings::PresetSettings::ITEMS,
        miyu_endpoint::settings::PermissionSettings::ITEMS,
    ]
    .concat();
    let config = miyu_endpoint::config::Config::load(
        &home.root,
        &alice(),
        None,
        items,
        miyu_endpoint::config::Environment::of(&[]),
    );
    let tools = Catalog::new(miyu_basesystem::tools(resources).expect("读得到工具的字"))
        .expect("出厂的工具登记得上");
    Arc::new(
        Core::new(
            home.root.clone(),
            ResourceRoot::at(resources.to_path_buf()),
            Arc::new(script.clone()),
            tools,
            None,
            alice(),
            TOKEN.to_string(),
        )
        .with_config(config),
    )
}

/// 日志里最后一条工具结果的字。
fn last_result(home: &Home, session: &str) -> String {
    home.log(session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ToolResult(result) => Some(
                result
                    .blocks
                    .iter()
                    .map(|block| match block {
                        Block::Text(Text { text }) => text.clone(),
                        _ => String::new(),
                    })
                    .collect::<String>(),
            ),
            _ => None,
        })
        .next_back()
        .expect("有工具结果")
}

#[tokio::test]
async fn an_upgraded_session_swaps_and_its_executor_takes_the_new_texts() {
    let home = Home::new();
    let resources = home.work.join("resources");
    copy_tree(&default_resources(), &resources);
    home.write(
        "home/alice/personas/miyu/prompts/persona.md",
        "You are Miyu.\n",
    );
    let forbidden = home.root.path().join("x.txt");
    let wrote = json!({"file_path": forbidden, "content": "x"}).to_string();
    let script = Script::new([
        Play::Says("嗯。"),
        Play::Says("好。"),
        Play::calls(&[("write", wrote.as_str())]),
        Play::Says("写不了。"),
    ]);
    let mut client = Client::connect(core_at(&home, &script, &resources));
    client.hello().await;
    let made = client
        .call(
            "c1",
            "session.create",
            json!({"cwd": "~", "persona": "miyu"}),
        )
        .await;
    let session = made["result"]["session"]
        .as_str()
        .expect("造出来了")
        .to_string();
    client.say("s1", &session, "hi").await;
    home.until_turns(&session, 1).await;

    // 程序升级了：两句核心的字变了。光是这样，下一个回合不换。
    std::fs::write(
        resources.join("core/permissions/forbidden.txt"),
        "UPGRADED: \"{path}\" is off limits.\n",
    )
    .expect("写得进");
    std::fs::write(
        resources.join("core/drivers/no-output.txt"),
        "UPGRADED nothing\n",
    )
    .expect("写得进");
    client.say("s2", &session, "在吗").await;
    home.until_turns(&session, 2).await;
    assert!(script.retexted().is_empty(), "光是升级不换");

    // 改了人格：下一个回合换上，执行器的字跟着换。
    home.write(
        "home/alice/personas/miyu/prompts/persona.md",
        "You are Miyu, softly.\n",
    );
    client.say("s3", &session, "写一个文件").await;
    home.until_turns(&session, 3).await;
    assert!(
        last_result(&home, &session).starts_with("UPGRADED: "),
        "{}",
        last_result(&home, &session)
    );
    let retexted = script.retexted();
    assert_eq!(retexted.len(), 1, "换了一次");
    assert_eq!(retexted[0].no_output(), "UPGRADED nothing\n");
}
