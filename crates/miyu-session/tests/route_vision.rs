//! 替看不了图的模型看图（`docs/blueprint/models.md`「怎么走」第十三条，施工 8-17）：会话的模型看不了图（资料里没有 `image`），
//! 人发来一张图：先经一次性入口问 `models.vision`，带着指令、人这一轮说的那句和这张图的字节；记下 `image.described`；主请求
//! 里图的位置是带标签的转述，不发图。同一张图下一轮不再问。会话的模型看得了图的不问、照发原图。没配 `models.vision` 的照旧
//! 是占位，主请求照发。
//!
//! 两台假服务器：`a` 是会话的模型 `a/m`（没写 `inputs`，看不了图），`b` 是看图的 `b/v`（`inputs` 有 `image`）。档案是空的、
//! 没有目录。

mod support;

use std::time::Duration;

use miyu_http::testkit::Server;
use miyu_kernel::block::{Block, Image, Text};
use miyu_kernel::event::{Body, ImageDescribed};
use miyu_kernel::id::{ContentHash, MediaType, ModelName, ProviderId};
use miyu_kernel::origin::By;
use miyu_kernel::session::Command;
use miyu_session::{Handle, Routes};
use miyu_store::blob::Blobs;
use support::routing::{configs, hellos, routes};
use support::{Home, alice_account, ask, until_turn_ends, watch};

/// `a/m` 是会话的模型；`m_inputs` 是它那一段另写的（看得了图的写 `inputs`）；`extra` 接在 `[models]` 那一段最后。
fn config(first: &Server, second: &Server, m_inputs: &str, extra: &str) -> String {
    format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.a.models.m]\nwindow = 100000\n{m_inputs}\n\
         [providers.b]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.b.models.v]\ninputs = [\"text\", \"image\"]\n\n\
         [models]\nchat = \"a/m\"\n{extra}",
        first.base_url, second.base_url
    )
}

/// 存进属主 blob 的一张「图」：四个字节 miyu，base64 是 bWl5dQ==。
fn stored(home: &Home) -> ContentHash {
    Blobs::new(home.root.blobs(&alice_account()))
        .put(b"miyu")
        .expect("存得进去")
}

/// 说一句带图的话，等这一轮说完。
async fn show(handle: &Handle, command: &str, words: &str, blob: &ContentHash) {
    let mut pushes = watch(handle).await;
    let blocks = vec![
        Block::Text(Text {
            text: words.to_string(),
        }),
        Block::Image(Image {
            blob: blob.clone(),
            name: None,
            media_type: MediaType::parse("image/png").expect("媒体类型合写法"),
            width: 1,
            height: 1,
        }),
    ];
    let send = Command::Send {
        blocks,
        urgent: false,
    };
    ask(handle, command, send).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

/// 一台服务器收到的请求体，照先后。
fn bodies(server: &Server) -> Vec<String> {
    server
        .received()
        .iter()
        .map(|received| String::from_utf8_lossy(&received.body).into_owned())
        .collect()
}

/// 会话日志里的转述：谁记的、带不带回合编号、写了什么。
fn described(home: &Home, handle: &Handle) -> Vec<(By, bool, ImageDescribed)> {
    home.log(handle.id())
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ImageDescribed(described) => Some((event.by, event.turn.is_some(), described)),
            _ => None,
        })
        .collect()
}

fn plain() -> Routes {
    routes(serde_json::json!({}), Duration::from_secs(5))
}

#[tokio::test]
async fn models_vision_describes_the_picture_and_the_request_carries_it() {
    let (first, second) = (
        Server::start(hellos(4)).await,
        Server::start(hellos(2)).await,
    );
    let mut home = Home::new();
    home.configs = configs(&config(&first, &second, "", "vision = \"b/v\"\n"), &[]);
    let routes = plain();
    let handle = home.create(&routes).await;
    let blob = stored(&home);
    show(&handle, "cmd-1", "图里第三行写的是什么？", &blob).await;
    let asked = bodies(&second);
    assert_eq!(asked.len(), 1, "问了一次看图的模型：{asked:?}");
    let ask: serde_json::Value = serde_json::from_str(&asked[0]).expect("请求是 JSON");
    assert_eq!(ask["model"], "v");
    assert!(ask.get("tools").is_none(), "不带工具：{ask}");
    let parts = ask["messages"][0]["content"].clone();
    let instruction = include_str!("../../../resources/core/vision/instruction.txt");
    let question = include_str!("../../../resources/core/vision/question.txt");
    assert_eq!(
        parts[0]["text"],
        format!("{instruction}{question}图里第三行写的是什么？"),
        "指令、那一行、人这一轮说的那句"
    );
    assert_eq!(
        parts[1]["image_url"]["url"],
        "data:image/png;base64,bWl5dQ=="
    );
    let [(by, in_turn, body)] = &described(&home, &handle)[..] else {
        panic!("记了一条转述");
    };
    assert_eq!((by, in_turn), (&By::Kernel, &false));
    assert_eq!(
        body,
        &ImageDescribed {
            blob: blob.clone(),
            endpoint: ProviderId::parse("b").unwrap(),
            model: ModelName::parse("v").unwrap(),
            text: "你好！".to_string(),
        }
    );
    let main = bodies(&first);
    let main = &main[0];
    assert!(
        main.contains(r#"<image-description>\n你好！\n</image-description>\n"#),
        "主请求里图的位置是带标签的转述：{main}"
    );
    assert!(!main.contains("base64"), "不发图：{main}");
    // 同一张图下一轮不再问。
    show(&handle, "cmd-2", "再看一眼", &blob).await;
    assert_eq!(bodies(&second).len(), 1, "同一张图只转述一次");
    assert_eq!(described(&home, &handle).len(), 1);
}

#[tokio::test]
async fn a_model_that_sees_gets_the_picture_and_no_one_is_asked() {
    let (first, second) = (
        Server::start(hellos(4)).await,
        Server::start(Vec::new()).await,
    );
    let mut home = Home::new();
    let sees = "inputs = [\"text\", \"image\"]\n";
    home.configs = configs(&config(&first, &second, sees, "vision = \"b/v\"\n"), &[]);
    let routes = plain();
    let handle = home.create(&routes).await;
    let blob = stored(&home);
    show(&handle, "cmd-1", "这是什么？", &blob).await;
    assert!(second.received().is_empty(), "看得了图的不转述");
    assert!(described(&home, &handle).is_empty());
    assert!(bodies(&first)[0].contains("data:image/png;base64,bWl5dQ=="));
}

#[tokio::test]
async fn without_models_vision_the_placeholder_goes_out() {
    let (first, second) = (
        Server::start(hellos(4)).await,
        Server::start(Vec::new()).await,
    );
    let mut home = Home::new();
    home.configs = configs(&config(&first, &second, "", ""), &[]);
    let routes = plain();
    let handle = home.create(&routes).await;
    let blob = stored(&home);
    show(&handle, "cmd-1", "这是什么？", &blob).await;
    assert!(described(&home, &handle).is_empty(), "没成的不记");
    let main = bodies(&first);
    assert!(
        main[0].contains("An image was attached here, but this model cannot view images."),
        "主请求照发，图写占位：{}",
        main[0]
    );
}

/// 池里有一个成员看不了图，钉住的、轮换的都算看不了：钉住的池这时钉着的是看得了图的那个，也先转述（出错会换到别的成员）。
#[tokio::test]
async fn a_pool_with_one_blind_member_is_blind() {
    for strategy in ["pin", "rotate"] {
        let (first, second) = (
            Server::start(hellos(4)).await,
            Server::start(hellos(1)).await,
        );
        let mut home = Home::new();
        let source = format!(
            "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.a.models.n]\ninputs = [\"text\", \"image\"]\n\n\
             [providers.b]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.b.models.v]\ninputs = [\"text\", \"image\"]\n\n\
             [pools.p]\nmodels = [\"a/n\", \"a/m\"]\nstrategy = \"{strategy}\"\n\n[models]\nchat = \"@p\"\nvision = \"b/v\"\n",
            first.base_url, second.base_url
        );
        home.configs = configs(&source, &[]);
        let routes = plain();
        let handle = home.create(&routes).await;
        let blob = stored(&home);
        show(&handle, "cmd-1", "这是什么？", &blob).await;
        assert_eq!(
            bodies(&second).len(),
            1,
            "{strategy}：池里有一个看不了图的就转述"
        );
        assert_eq!(described(&home, &handle).len(), 1, "{strategy}");
    }
}

/// 钉住的池出错换到看得了图的成员、成了以后钉到它：限额跟着换成它的；下一轮开始照整个池重算，看不看得了图照整个池，下一轮的
/// 图照样先转述。
#[tokio::test]
async fn a_pinned_pool_is_blind_again_next_turn_after_moving_to_a_member_that_sees() {
    let mut replies = vec![support::calling::limited()];
    replies.extend(hellos(4));
    let (first, second) = (Server::start(replies).await, Server::start(hellos(1)).await);
    let mut home = Home::new();
    let source = format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.a.models.n]\ninputs = [\"text\", \"image\"]\n\n\
         [providers.b]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.b.models.v]\ninputs = [\"text\", \"image\"]\n\n\
         [pools.p]\nmodels = [\"a/m\", \"a/n\"]\nstrategy = \"pin\"\n\n[models]\nchat = \"@p\"\nvision = \"b/v\"\n",
        first.base_url, second.base_url
    );
    home.configs = configs(&source, &[]);
    let routes = plain();
    let handle = home.create(&routes).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", support::say("hi"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let models: Vec<String> = bodies(&first)
        .iter()
        .map(|body| serde_json::from_str::<serde_json::Value>(body).unwrap()["model"].to_string())
        .collect();
    assert_eq!(
        models[..2],
        ["\"m\"", "\"n\""],
        "m 限速、换到 n：{models:?}"
    );
    let blob = stored(&home);
    show(&handle, "cmd-2", "这是什么？", &blob).await;
    assert_eq!(bodies(&second).len(), 1, "钉到看得了图的 n 以后照样先转述");
}
