//! 随机测试里重读的结果（施工 6-5）：另用一串随机数送，原来那串输入不跟着错开。

use super::*;
use crate::id::ContentHash;
use crate::session::Reread;

/// 重读的结果：交出了重读的，四回里有三回送回来，一个一项，读到了的原文长短随机，有的太大、有的读不到；不送的，摘要
/// 请求照没有候选写。没交出的时候八十回里一回送一个过时的（该不理），一百五十回里一回交一份原文（载入以后那种，该
/// 什么都不出）。
pub(super) fn some_reread(rng: &mut Rng, watch: &Watch) -> Option<Input> {
    match watch.reread_pending() {
        Some((seen, paths)) if rng.below(4) > 0 => {
            let files = paths
                .iter()
                .map(|_| match rng.below(5) {
                    0 => Reread::TooLarge,
                    1 => Reread::Unreadable,
                    _ => {
                        let text = "x".repeat(1 + rng.below(120) as usize);
                        Reread::Read {
                            blob: ContentHash::of(text.as_bytes()),
                            text,
                        }
                    }
                })
                .collect();
            Some(Input::Reread {
                at: at(58),
                seen,
                files,
            })
        }
        None if rng.below(80) == 0 => Some(Input::Reread {
            at: at(58),
            seen: Seq::new(9_999).unwrap(),
            files: Vec::new(),
        }),
        None if rng.below(150) == 0 => Some(Input::Recalled {
            texts: BTreeMap::from([(ContentHash::of(b"y"), "y".to_string())]),
        }),
        _ => None,
    }
}
