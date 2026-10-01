//! `image.described` 的测试（施工 8-17）：图纸上的一行读写一字不差、认得出种类；四格都要写；坏的报错说清是哪一种。

use super::*;
use crate::event::{Body, Event};
use crate::test_support::{event_line, read_body, rejected};

/// 图纸上的那一份 `body`（`docs/designs/samples/events/image.described.jsonl` 的那一条）。
const BODY: &str = r#"{"blob":"sha256:5f2c8e1a9b3d7c4e6f0a2b8d1c5e9f3a7b4d6c0e8f2a1b5c9d3e7f4a6b0c8d2e","endpoint":"bigmodel","model":"glm-5.3-flash","text":"A terminal window.\nThe last line reads: test result: FAILED."}"#;

#[test]
fn the_description_from_the_drawing_round_trips() {
    match read_body("image.described", BODY) {
        Body::ImageDescribed(described) => assert_eq!(
            described,
            ImageDescribed {
                blob: ContentHash::parse(
                    "sha256:5f2c8e1a9b3d7c4e6f0a2b8d1c5e9f3a7b4d6c0e8f2a1b5c9d3e7f4a6b0c8d2e"
                )
                .unwrap(),
                endpoint: ProviderId::parse("bigmodel").unwrap(),
                model: ModelName::parse("glm-5.3-flash").unwrap(),
                text: "A terminal window.\nThe last line reads: test result: FAILED.".to_string(),
            }
        ),
        other => panic!("{other:?}"),
    }
    let line = event_line("image.described", BODY);
    assert_eq!(Event::from_line(&line).unwrap().to_line(), line);
}

#[test]
fn broken_descriptions_say_which_kind() {
    for body in [
        r#"{"endpoint":"bigmodel","model":"glm-5.3-flash","text":"a"}"#,
        r#"{"blob":"sha256:5f2c","endpoint":"bigmodel","model":"glm-5.3-flash","text":"a"}"#,
        r#"{"blob":"sha256:5f2c8e1a9b3d7c4e6f0a2b8d1c5e9f3a7b4d6c0e8f2a1b5c9d3e7f4a6b0c8d2e","model":"glm-5.3-flash","text":"a"}"#,
        r#"{"blob":"sha256:5f2c8e1a9b3d7c4e6f0a2b8d1c5e9f3a7b4d6c0e8f2a1b5c9d3e7f4a6b0c8d2e","endpoint":"bigmodel","text":"a"}"#,
        r#"{"blob":"sha256:5f2c8e1a9b3d7c4e6f0a2b8d1c5e9f3a7b4d6c0e8f2a1b5c9d3e7f4a6b0c8d2e","endpoint":"bigmodel","model":"glm-5.3-flash"}"#,
        r#"{"blob":"sha256:5f2c8e1a9b3d7c4e6f0a2b8d1c5e9f3a7b4d6c0e8f2a1b5c9d3e7f4a6b0c8d2e","endpoint":"bigmodel","model":"glm-5.3-flash","text":7}"#,
    ] {
        rejected::<Event>(
            &event_line("image.described", body),
            "body of image.described not readable",
        );
    }
}
