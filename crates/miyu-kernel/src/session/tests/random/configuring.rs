//! 随机测试里换模型（施工 8-10）：另用一串随机数、另一串命令编号，夹在原来的输入之间、不占名额，只在四个种子里的一个：
//! 原来那串输入不跟着错开，别的种子照原来的走。什么时候来都收，回合进行中的带上回合；五回里一回把上一次的编号再送一遍
//! （换成别的也不再生效）。挂接点的结果偶尔带着退回：多半照会话现在的引用退回 `a/m`，偶尔对不上（看守查对不上的不记）。
//!
//! 换思考强度（施工 8-18）也在这个种子里，再另用一串随机数、一串命令编号（`effort-<n>`）：二十回里一回，两个模型、几档和
//! 清掉，三回里一回连同换模型一起来；一样的那一样不写，看守照自己记下的日志判。

use super::*;
use crate::event::Effort;
use crate::session::Replaced;

/// 换成的几个引用：有和会话现在一样的时候。
const MODELS: [&str; 3] = ["a/m", "@free", "b/n"];

/// 换一次模型：二十回里一回。
pub(super) fn some_configure(rng: &mut Rng, ids: &mut u64) -> Option<Input> {
    if rng.below(20) != 0 {
        return None;
    }
    let again = *ids > 0 && rng.below(5) == 0;
    let n = match again {
        true => *ids,
        false => next_command(ids),
    };
    let model = MODELS[rng.below(3) as usize];
    Some(Input::Command(Received {
        id: CommandId::parse(&format!("model-{n}")).unwrap(),
        by: alice(),
        at: at(30),
        command: Command::Configure {
            model: Some(model.to_string()),
            effort: None,
        },
    }))
}

/// 换思考强度的几个模型、几档：有清掉的（`None`），有和会话记着的一样的时候。
const EFFORT_MODELS: [&str; 2] = ["a/m", "b/n"];
const LEVELS: [Option<&str>; 3] = [Some("high"), Some("off"), None];

/// 换一次思考强度：二十回里一回，三回里一回连同换模型一起来（施工 8-18）。五回里一回把上一次的编号再送一遍。
pub(super) fn some_effort(rng: &mut Rng, ids: &mut u64) -> Option<Input> {
    if rng.below(20) != 0 {
        return None;
    }
    let again = *ids > 0 && rng.below(5) == 0;
    let n = match again {
        true => *ids,
        false => next_command(ids),
    };
    let effort = Effort {
        model: EFFORT_MODELS[rng.below(2) as usize].to_string(),
        level: LEVELS[rng.below(3) as usize].map(str::to_string),
    };
    let model = (rng.below(3) == 0).then(|| MODELS[rng.below(3) as usize].to_string());
    Some(Input::Command(Received {
        id: CommandId::parse(&format!("effort-{n}")).unwrap(),
        by: alice(),
        at: at(30),
        command: Command::Configure {
            model,
            effort: Some(effort),
        },
    }))
}

/// 挂接点的结果三回里一回带着退回：原来的四回里三回是会话现在的引用（没有的写 `b/n`），退回的是 `a/m`。别的输入原样交回。
pub(super) fn with_fallback(rng: &mut Rng, watch: &Watch, input: Input) -> Input {
    match input {
        Input::TurnStartHooksDone {
            at,
            turn,
            injected,
            replaced: None,
        } if rng.below(3) == 0 => {
            let from = match rng.below(4) {
                0 => "b/n",
                _ => watch.reference().unwrap_or("b/n"),
            };
            Input::TurnStartHooksDone {
                at,
                turn,
                injected,
                replaced: Some(Replaced {
                    from: from.to_string(),
                    to: "a/m".to_string(),
                }),
            }
        }
        input => input,
    }
}
