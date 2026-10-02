//! 请求形状探针：看不了图的那一张脸（施工 8-17，`docs/blueprint/kernel/request.md`「替它看的图」，`models.md`「怎么走」第十三
//! 条）。真内核照剧本跑，组装、给模型看的字都是出厂的，限额说看不了图：
//!
//! 1. 人附一张带名字的截图问第三行，先转述（转述请求带着人这句话），再请求：图的位置是带名字的标签和转述；她读了文件再答，
//!    同一轮里不再转述。
//! 2. 人又附一张不带名字的图，转述没成：这一轮写占位，主请求照发。
//! 3. 人再问一句，没有新图：上一轮没成的那张下一轮再转，成了；那条消息从占位换成转述（登记在案的改写），截图的转述照旧。
//!
//! 和存档（`docs/designs/samples/probe/vision/`）逐字节比：`requests/`、`openai-chat/` 是主请求，`describes/` 是转述请求。
//! 主请求照样查五条性质。

mod support;

use miyu_kernel::block::{Block, Image, Text};
use miyu_kernel::event::Body;
use miyu_kernel::id::{ContentHash, FileName, MediaType};
use miyu_kernel::request::Message;
use miyu_kernel::session::Limits;
use miyu_kernel::testkit::{Line, Play, Stage, model};
use support::{check, files, matches_the_archive, sent, stage, vision, wire};

/// 截图的转述。
const SHOT: &str = "A terminal window showing the end of a cargo test run in red. The last lines read:\nfailures:\n    support::temp_dir_is_canonical\ntest result: FAILED. 214 passed; 1 failed; 0 ignored";

/// 那张不带名字的图的转述。
const CHART: &str =
    "A bar chart of build times per platform: Linux 4 min, macOS 9 min, Windows 7 min.";

/// 一张图：`content` 当字节，带名字的写上名字。
fn picture(content: &[u8], name: Option<&str>) -> Block {
    Block::Image(Image {
        blob: ContentHash::of(content),
        name: name.map(|name| FileName::parse(name).expect("文件名合写法")),
        media_type: MediaType::parse("image/png").expect("媒体类型合写法"),
        width: 1280,
        height: 720,
    })
}

fn text(words: &str) -> Block {
    Block::Text(Text {
        text: words.to_string(),
    })
}

/// 剧本。
fn script() -> Stage {
    let mut s = stage();
    s.limits_with(Limits {
        model: model(),
        window: None,
        max_output: None,
        images: None,
        blind: true,
    });
    // 1. 人附一张截图问第三行：先转述，再请求；她读了文件答。
    s.vision([Some(SHOT)]);
    s.model([
        Line::calls(
            "我看一下那个测试。",
            &[("read", r#"{"path":"tests/support/mod.rs"}"#)],
        ),
        Line::says("第三行是失败的那个测试：support::temp_dir_is_canonical。"),
    ]);
    s.tools([Play::done(
        "fn temp_dir() -> PathBuf { std::env::temp_dir() }",
    )]);
    s.send(vec![
        text("第三行写的是什么？"),
        picture(b"screenshot", Some("截图.png")),
    ]);
    // 2. 人又附一张不带名字的图，转述没成：写占位。
    s.advance(2);
    s.vision([None]);
    s.model([Line::says("这张图我看不到，能说说是什么吗？")]);
    s.send(vec![text("这张呢？"), picture(b"chart", None)]);
    // 3. 人再问一句：那张图下一轮再转，成了。
    s.advance(2);
    s.vision([Some(CHART)]);
    s.model([Line::says("macOS 最慢，九分钟。")]);
    s.say("哪个平台最慢？");
    s
}

#[test]
fn the_vision_session_matches_the_archive() {
    matches_the_archive("vision", &script());
}

#[test]
fn the_same_script_gives_the_same_bytes() {
    assert_eq!(files(&script()), files(&script()));
}

/// 五条性质照查；转述请求带着人这一轮说的那句；线上图的位置是带名字的标签和转述，没成的是占位，下一轮换成转述。
#[test]
fn descriptions_take_the_pictures_places() {
    let session = script();
    if let Err(why) = check(&sent(&session)) {
        panic!("{why}");
    }
    let asked: Vec<String> = session
        .describes()
        .iter()
        .map(|(_, request)| match &request.messages[..] {
            [Message::User { blocks }] => match &blocks[..] {
                [Block::Text(text), Block::Image(image)] => {
                    assert_eq!(image.name, None, "转述只看画面，不带名字");
                    text.text.clone()
                }
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        })
        .collect();
    let vision = vision();
    assert_eq!(
        asked,
        [
            format!(
                "{}{}第三行写的是什么？",
                vision.instruction, vision.question
            ),
            format!("{}{}这张呢？", vision.instruction, vision.question),
            format!("{}{}哪个平台最慢？", vision.instruction, vision.question),
        ]
    );
    let described = session
        .log()
        .iter()
        .filter(|event| matches!(event.body, Body::ImageDescribed(_)))
        .count();
    assert_eq!(described, 2, "没成的不记");
    let requests = session.requests();
    let first = String::from_utf8(wire(&requests[0].1).body).unwrap();
    assert!(
        first.contains(&format!(
            "<image-description name=\\\"截图.png\\\">\\n{}\\n</image-description>\\n",
            SHOT.replace('\n', "\\n")
        )),
        "{first}"
    );
    let failed = String::from_utf8(wire(&requests[2].1).body).unwrap();
    assert!(
        failed.contains("An image was attached here, but this model cannot view images."),
        "{failed}"
    );
    let last = String::from_utf8(wire(&requests[3].1).body).unwrap();
    assert!(
        last.contains(&format!(
            "<image-description>\\n{CHART}\\n</image-description>\\n"
        )),
        "{last}"
    );
}
