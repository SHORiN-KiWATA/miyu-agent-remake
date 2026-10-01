//! 随机测试里替它看图（施工 8-17）：另用一串随机数、另一串命令编号，夹在原来的输入之间、不占名额，只在八个种子里的一个：
//! 原来那串输入不跟着错开，别的种子照原来的走。人偶尔发一条带图的消息（三张图里挑一张，重的也有）；限额偶尔交成看不了
//! 图的；在路上的转述多半很快回来，四回里一回没成；偶尔来一个对不上的转述，该不理。

use super::*;
use crate::block::Image;
use crate::id::MediaType;
use crate::session::Limits;

/// 三张图里的第 `k` 张。
fn picture(k: u64) -> Image {
    Image {
        blob: ContentHash::of(format!("picture {k}").as_bytes()),
        name: None,
        media_type: MediaType::parse("image/png").unwrap(),
        width: 640,
        height: 480,
    }
}

/// 一次替它看图的输入，没有的是 `None`：在路上的三回里一回送回来，发带图的消息十二回里一回，交限额二十回里一回，对不上的
/// 转述六十回里一回。
pub(super) fn some_sight(rng: &mut Rng, watch: &Watch, ids: &mut u64) -> Option<Input> {
    if let Some(blob) = watch.sight.looking.keys().next().cloned()
        && rng.below(3) == 0
    {
        let seen = (rng.below(4) > 0).then(|| (model(), format!("picture {}", rng.below(1000))));
        return Some(Input::Described {
            at: at(47),
            blob,
            seen,
        });
    }
    match rng.below(60) {
        0..=4 => {
            let n = next_command(ids);
            Some(Input::Command(Received {
                id: CommandId::parse(&format!("sight-{n}")).unwrap(),
                by: alice(),
                at: at(31),
                command: Command::Send {
                    blocks: vec![
                        Block::Text(Text {
                            text: "看看这张图".to_string(),
                        }),
                        Block::Image(picture(rng.below(3))),
                    ],
                    urgent: false,
                },
            }))
        }
        5..=7 => Some(Input::Limits(Limits {
            model: model(),
            window: (rng.below(3) == 0).then_some(300),
            max_output: None,
            images: None,
            blind: rng.below(4) > 0,
        })),
        8 => Some(Input::Described {
            at: at(47),
            blob: ContentHash::of(b"never asked"),
            seen: Some((model(), "stray".to_string())),
        }),
        _ => None,
    }
}

/// 替它看图的模型。
fn model() -> Model {
    Model {
        endpoint: ProviderId::parse("bigmodel").unwrap(),
        model: ModelName::parse("glm-5.3-flash").unwrap(),
    }
}
