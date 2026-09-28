//! 随机测试里改回文件的结局（施工 4-7 上）：另用一串随机数送，原来那串输入不跟着错开。

use super::*;
use crate::event::RestoreOutcome;

/// 改回文件的结局，另用一串随机数：在改的时候六回里有四回送回来，一步一项，四步里有一步没对上；一回送来一句话
/// （该拒，施工 4-7 下：原来靠碰运气碰上）；没在改的时候六十回里一回送一个过时的（该不理）。
pub(super) fn some_restored(rng: &mut Rng, watch: &Watch, next_id: &mut u64) -> Option<Input> {
    match &watch.restoring.pending {
        Some(_) if rng.below(6) == 0 => Some(send(next_command(next_id), "hi")),
        Some(steps) if rng.below(5) > 0 => {
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
