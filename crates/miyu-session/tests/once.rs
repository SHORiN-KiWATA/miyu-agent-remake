//! 模型调用口的一次性入口（`docs/blueprint/models.md`「怎么走」第十二条，施工 8-20）：模型、`@池`、不写照 `models.chat`；
//! system 和几条消息照先后发、不带工具、`max_tokens` 照写的发；带图照字节发、模型不收图的不发；四种出错；配置的默认强度、温度（施工 8-22）；
//! 取不到 key 的当场 `no_model`；429 当场换池里的下一个、说到一半断了也换、只有一个候选的不再来、最多换 5 次（施工 8-25 起一家
//! 一个 key，几个候选只来自池）。
//!
//! 假服务器在本机回环上，档案是空的、没有目录：资料全照手写的。

use std::time::Duration;

use serde_json::json;

use crate::support::calling::{
    asking, bearer, blobs, body, entry, frozen, keyed_config, limited, user,
};
use crate::support::routing::{cut_after, hellos, routes};
use miyu_http::testkit::{Reply, Server};
use miyu_kernel::block::{Block, Image, Text};
use miyu_kernel::event::{ErrorClass, Usage};
use miyu_kernel::id::MediaType;
use miyu_kernel::request::Message;
use miyu_session::{Answer, Ask, Unanswered};

/// 说「你好！」的那一份流报的用量：输入 2000（命中 1920），输出 3。
const HELLO_USAGE: Usage = Usage {
    uncached: 80,
    cache_read: 1920,
    cache_write: 0,
    output: 3,
};

fn plain() -> miyu_session::Routes {
    routes(json!({}), Duration::from_secs(60))
}

/// 两家：`a` 在 `first`、`b` 在 `second`，都不带 key；池 `p` 只有 `b/n`；`models.chat` 是 `a/m`。`extra` 接在最后。
fn two(first: &Server, second: &Server, extra: &str) -> String {
    format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.b]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n\
         [pools.p]\nmodels = [\"b/n\"]\n\n[models]\nchat = \"a/m\"\n{extra}",
        first.base_url, second.base_url
    )
}

#[tokio::test]
async fn a_model_a_pool_and_models_chat_each_answer_with_what_was_really_used() {
    let first = Server::start(hellos(2)).await;
    let second = Server::start(hellos(1)).await;
    let config = frozen(&two(&first, &second, ""), &[]);
    let routes = plain();
    let entry = entry(&routes);
    let (_scratch, blobs) = blobs();
    let answered = entry
        .call(&config, &blobs, asking(None, "platform", "hi"))
        .await;
    let answer = |provider: &str, model: &str| Answer {
        text: "你好！".to_string(),
        provider: provider.to_string(),
        model: model.to_string(),
        usage: Some(HELLO_USAGE),
    };
    assert_eq!(answered, Ok(answer("a", "m")), "不写照 models.chat");
    let answered = entry
        .call(&config, &blobs, asking(Some("a/x"), "platform", "hi"))
        .await;
    assert_eq!(answered, Ok(answer("a", "x")), "写了的模型");
    let answered = entry
        .call(&config, &blobs, asking(Some("@p"), "platform", "hi"))
        .await;
    assert_eq!(answered, Ok(answer("b", "n")), "池照它的成员");
    assert_eq!(body(&first, 0)["model"], "m");
    assert_eq!(body(&first, 1)["model"], "x");
    assert_eq!(body(&second, 0)["model"], "n");
}

#[tokio::test]
async fn the_messages_go_in_order_without_tools_and_with_the_output_limit() {
    let first = Server::start(hellos(2)).await;
    let second = Server::start(Vec::new()).await;
    let config = frozen(&two(&first, &second, ""), &[]);
    let routes = plain();
    let (_scratch, blobs) = blobs();
    let ask = Ask {
        model: None,
        purpose: "platform".to_string(),
        system: "Be brief.".to_string(),
        messages: vec![
            user("q1"),
            Message::Assistant {
                blocks: vec![Block::Text(Text {
                    text: "a1".to_string(),
                })],
            },
            user("q2"),
        ],
        max_tokens: Some(64),
        owner: miyu_kernel::id::AccountId::parse("admin").unwrap(),
    };
    let answered = entry(&routes).call(&config, &blobs, ask).await;
    assert!(answered.is_ok(), "{answered:?}");
    let sent = body(&first, 0);
    assert_eq!(
        sent["messages"],
        json!([
            {"role": "system", "content": "Be brief."},
            {"role": "user", "content": "q1"},
            {"role": "assistant", "content": "a1"},
            {"role": "user", "content": "q2"},
        ])
    );
    assert_eq!(sent["max_tokens"], 64);
    assert!(sent.get("tools").is_none(), "不带工具：{sent}");
    entry(&routes)
        .call(&config, &blobs, asking(None, "platform", "hi"))
        .await
        .expect("答得上来");
    assert!(body(&first, 1).get("max_tokens").is_none(), "没写的不限");
}

#[tokio::test]
async fn an_image_goes_out_as_its_bytes_and_a_model_without_images_is_not_sent() {
    let first = Server::start(hellos(1)).await;
    let second = Server::start(Vec::new()).await;
    let extra = "\n[providers.a.models.v]\ninputs = [\"text\", \"image\"]\n";
    let config = frozen(&two(&first, &second, extra), &[]);
    let routes = plain();
    let (_scratch, blobs) = blobs();
    // 「图」是四个字节 miyu，base64 是 bWl5dQ==。
    let blob = blobs.put(b"miyu").expect("存得进去");
    let looking = |model: &str| Ask {
        messages: vec![Message::User {
            blocks: vec![
                Block::Text(Text {
                    text: "What is this?".to_string(),
                }),
                Block::Image(Image {
                    blob: blob.clone(),
                    name: None,
                    media_type: MediaType::parse("image/png").expect("合写法"),
                    width: 1,
                    height: 1,
                }),
            ],
        }],
        ..asking(Some(model), "vision", "")
    };
    let answered = entry(&routes).call(&config, &blobs, looking("a/v")).await;
    assert!(answered.is_ok(), "{answered:?}");
    let sent = String::from_utf8_lossy(&first.received()[0].body).to_string();
    assert!(
        sent.contains("data:image/png;base64,bWl5dQ=="),
        "图照字节发：{sent}"
    );
    let refused = entry(&routes).call(&config, &blobs, looking("a/m")).await;
    assert_eq!(
        refused,
        Err(Unanswered::Failed(miyu_kernel::event::CallError {
            class: ErrorClass::Unclassified,
            message: "model \"a/m\" does not take images".to_string(),
            status: None,
        }))
    );
    assert_eq!(first.received().len(), 1, "不收图的不发");
}

#[tokio::test]
async fn unknown_model_and_no_model_are_said_without_sending() {
    let first = Server::start(Vec::new()).await;
    let second = Server::start(Vec::new()).await;
    let config = frozen(&two(&first, &second, ""), &[]);
    let routes = plain();
    let (_scratch, blobs) = blobs();
    let entry = entry(&routes);
    for (model, why) in [
        ("nope/x", "no provider \"nope\""),
        ("@missing", "no pool \"missing\""),
        ("plain", "\"plain\" is not a model or a pool"),
    ] {
        let answered = entry
            .call(&config, &blobs, asking(Some(model), "platform", "hi"))
            .await;
        assert_eq!(answered, Err(Unanswered::UnknownModel(why.to_string())));
    }
    let bare = frozen("", &[]);
    let answered = entry
        .call(&bare, &blobs, asking(None, "platform", "hi"))
        .await;
    assert_eq!(
        answered,
        Err(Unanswered::NoModel(
            "no model configured: set models.chat".to_string()
        ))
    );
    let unset = keyed_config(&first.base_url, 1, &[]);
    let answered = entry
        .call(&unset, &blobs, asking(None, "platform", "hi"))
        .await;
    assert_eq!(
        answered,
        Err(Unanswered::NoModel(
            "provider \"a1\" has no usable key".to_string()
        ))
    );
    assert!(first.received().is_empty() && second.received().is_empty());
}

#[tokio::test]
async fn a_failure_with_one_candidate_is_handed_back_and_all_cooling_is_not_sent() {
    let unauthorized = Reply::error(
        401,
        &[],
        r#"{"error":{"message":"bad key","type":"authentication_error"}}"#,
    );
    let lone = Server::start(vec![unauthorized, limited()]).await;
    let (_scratch, blobs) = blobs();
    let one = keyed_config(&lone.base_url, 1, &[1]);
    let answered = entry(&plain())
        .call(&one, &blobs, asking(None, "platform", "hi"))
        .await;
    let Err(Unanswered::Failed(error)) = answered else {
        panic!("出错交回：{answered:?}");
    };
    assert_eq!(
        (&error.class, error.status),
        (&ErrorClass::Auth, Some(401)),
        "{error:?}"
    );
    assert!(error.message.contains("bad key"), "{error:?}");
    assert_eq!(lone.received().len(), 1);
    // 池里两家都撞了 429：换了一次以后只剩等，不等、交回；再叫一次，全在冷却，不发。
    let pair = Server::start(vec![limited(), limited(), limited()]).await;
    let entry = entry(&plain());
    let two = keyed_config(&pair.base_url, 2, &[1, 2]);
    let answered = entry
        .call(&two, &blobs, asking(None, "platform", "hi"))
        .await;
    assert!(
        matches!(&answered, Err(Unanswered::Failed(error)) if error.class == ErrorClass::RateLimited),
        "{answered:?}"
    );
    assert_eq!(pair.received().len(), 2, "换了一次");
    let answered = entry
        .call(&two, &blobs, asking(None, "platform", "hi"))
        .await;
    let Err(Unanswered::Cooling { message, wait_ms }) = answered else {
        panic!("全在冷却：{answered:?}");
    };
    assert!(
        message.starts_with("all candidates cooling: ")
            && message.contains("a1/m rate_limited until ")
            && message.contains("a2/m rate_limited until "),
        "{message}"
    );
    assert!(wait_ms > 0);
    assert_eq!(pair.received().len(), 2, "全在冷却的不发");
}

#[tokio::test]
async fn the_default_effort_of_the_model_goes_out() {
    let first = Server::start(hellos(1)).await;
    let second = Server::start(Vec::new()).await;
    let extra = "\n[providers.a.models.m]\nreasoning = [\"low\", \"high\"]\neffort = \"high\"\n";
    let config = frozen(&two(&first, &second, extra), &[]);
    let routes = plain();
    let (_scratch, blobs) = blobs();
    entry(&routes)
        .call(&config, &blobs, asking(None, "platform", "hi"))
        .await
        .expect("答得上来");
    assert_eq!(body(&first, 0)["reasoning_effort"], "high");
}

/// 温度（施工 8-22，「怎么走」第十四条第 5 条）：一次性入口照真发的那个模型配置的默认带，和会话入口一样。
#[tokio::test]
async fn the_default_temperature_of_the_model_goes_out() {
    let first = Server::start(hellos(1)).await;
    let second = Server::start(Vec::new()).await;
    let extra = "\n[providers.a.models.m]\ntemperature = 0.3\n";
    let config = frozen(&two(&first, &second, extra), &[]);
    let routes = plain();
    let (_scratch, blobs) = blobs();
    entry(&routes)
        .call(&config, &blobs, asking(None, "platform", "hi"))
        .await
        .expect("答得上来");
    assert_eq!(body(&first, 0)["temperature"], 0.3);
}

#[tokio::test]
async fn a_rate_limit_or_a_cut_reply_switches_to_the_next_member_at_once() {
    let key = |n: usize| Some(format!("Bearer sk-{n}"));
    let (_scratch, blobs) = blobs();
    for first in [limited(), cut_after(2)] {
        let mut replies = vec![first];
        replies.extend(hellos(1));
        let server = Server::start(replies).await;
        let config = keyed_config(&server.base_url, 2, &[1, 2]);
        let answered = entry(&plain())
            .call(&config, &blobs, asking(None, "platform", "hi"))
            .await;
        // 说到一半断了的：半截不要，换到池里的另一个整段重来。
        assert_eq!(answered.map(|answer| answer.text), Ok("你好！".to_string()));
        assert_eq!(server.received().len(), 2);
        assert_eq!((bearer(&server, 0), bearer(&server, 1)), (key(1), key(2)));
    }
}

#[tokio::test]
async fn a_single_candidate_is_not_tried_again_and_switching_stops_after_five() {
    let (_scratch, blobs) = blobs();
    let lone = Server::start(vec![limited(), limited()]).await;
    let keyless = format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[models]\nchat = \"a/m\"\n",
        lone.base_url
    );
    let answered = entry(&plain())
        .call(
            &frozen(&keyless, &[]),
            &blobs,
            asking(None, "platform", "hi"),
        )
        .await;
    assert!(
        matches!(answered, Err(Unanswered::Failed(_))),
        "{answered:?}"
    );
    assert_eq!(lone.received().len(), 1, "只有一个候选：不再来");
    let many = Server::start((0..8).map(|_| limited()).collect()).await;
    let seven = keyed_config(&many.base_url, 7, &[1, 2, 3, 4, 5, 6, 7]);
    let answered = entry(&plain())
        .call(&seven, &blobs, asking(None, "platform", "hi"))
        .await;
    assert!(
        matches!(answered, Err(Unanswered::Failed(_))),
        "{answered:?}"
    );
    assert_eq!(many.received().len(), 6, "一次加换 5 次");
}

#[tokio::test]
async fn a_success_clears_the_cooling_for_both_entries() {
    let mut replies = vec![limited()];
    replies.extend(hellos(2));
    let server = Server::start(replies).await;
    let routes = plain();
    let entry = entry(&routes);
    let (_scratch, blobs) = blobs();
    // 只有一家：撞了 429 记冷却（30 秒）；只有一个候选，冷着也照发，成了就清掉。
    let one = keyed_config(&server.base_url, 1, &[1]);
    for expected_ok in [false, true] {
        let answered = entry
            .call(&one, &blobs, asking(None, "platform", "hi"))
            .await;
        assert_eq!(answered.is_ok(), expected_ok, "{answered:?}");
    }
    // 池里两家、钉着的是第一家（就是上面那一家）：已经不在冷却了，照钉着的发。
    let two = keyed_config(&server.base_url, 2, &[1, 2]);
    entry
        .call(&two, &blobs, asking(None, "platform", "hi"))
        .await
        .expect("答得上来");
    assert_eq!(bearer(&server, 2), Some("Bearer sk-1".to_string()));
}
