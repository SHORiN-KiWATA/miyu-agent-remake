//! 随机测试里改回文件的结局（施工 4-7 上）：另用一串随机数送，原来那串输入不跟着错开。

use super::*;
use crate::event::RestoreOutcome;

/// 改回文件的结局，另用一串随机数：在改的时候三回里有两回送回来，一步一项，四步里有一步没对上；没在改的时候
/// 六十回里一回送一个过时的（该不理）。
pub(super) fn some_restored(rng: &mut Rng, watch: &Watch) -> Option<Input> {
    match &watch.restoring.pending {
        Some(steps) if rng.below(3) > 0 => {
            let files = steps
                .iter()
                .map(|step| {
                    let mut done = step.restored();
                    if rng.below(4) == 0 {
                        done.outcome = RestoreOutcome::Changed;
                        done.found = Some(ContentHash::of(b"someone else"));
                    }
                    done
                })
                .collect();
            Some(Input::Restored { at: at(58), files })
        }
        None if rng.below(60) == 0 => Some(Input::Restored {
            at: at(58),
            files: Vec::new(),
        }),
        _ => None,
    }
}
