//! 随机测试里和等人回答有关的几样输入：执行前的链的结论，人对确认的回答，在跑的调用问人，人对
//! 一组题的回答（`docs/designs/02-内核.md` 第六节「确认怎么走」「提问怎么走」）。

use super::*;

/// 链的一个结论：多半是交给了链的那几个，偶尔是对不上的。多半放行，有时要问人、拒绝。问人时
/// 要的多半是工具本来的那一类，读的偶尔要联网、要写入；一半提了放行规则。
pub(super) fn some_verdict(rng: &mut Rng, watch: &Watch) -> Input {
    let guarding: Vec<CallId> = watch.approvals.guarding.iter().copied().collect();
    let known = rng.below(6) > 0;
    let call_id = if known {
        guarding[rng.below(guarding.len() as u64) as usize]
    } else {
        CallId::new(seq(1 + rng.below(watch.last())), 1).unwrap()
    };
    let module = ModuleId::parse("permissions").unwrap();
    // 没人能确认的会话里，链多问几次人：内核要当场拒绝的就是这些。
    let asks = if watch.approvals.attended {
        12..=16
    } else {
        9..=16
    };
    let verdict = match rng.below(20) {
        k if asks.contains(&k) => Verdict::Ask {
            module,
            access: match (known && watch.name_of(call_id) == "write", rng.below(6)) {
                (true, _) | (false, 0) => Access::Write,
                (false, 1 | 2) => Access::Network,
                _ => Access::Read,
            },
            rule: (rng.below(2) == 0).then(|| serde_json::from_str::<RawJson>("{}").unwrap()),
            detail: None,
        },
        0..=11 => Verdict::Allow,
        _ => Verdict::Deny {
            module,
            text: "blocked".to_string(),
        },
    };
    Input::ToolGuarded {
        at: at(48),
        call_id,
        verdict,
    }
}

/// 一个回答：多半回答在等的那几个，偶尔是对不上的；选项多半认识，拒绝的有时带理由，也有空的理由。
pub(super) fn some_answer(rng: &mut Rng, watch: &Watch, next_id: &mut u64) -> Input {
    let asking: Vec<CallId> = watch.approvals.asking.iter().copied().collect();
    let call_id = if rng.below(8) > 0 {
        asking[rng.below(asking.len() as u64) as usize]
    } else {
        CallId::new(seq(1 + rng.below(watch.last())), 1).unwrap()
    };
    let decision = match rng.below(12) {
        0..=3 => Decision::Once,
        4 | 5 => Decision::Session,
        6 => Decision::Workspace,
        7..=10 => Decision::Deny,
        _ => Decision::Other("maybe".to_string()),
    };
    let reason = match rng.below(4) {
        0 => Some("别动"),
        1 => Some("  "),
        _ => None,
    };
    answer(next_command(next_id), call_id, decision, reason)
}

/// 在跑的一个调用问人：一两道题，每道两个选项，一半能多选。
pub(super) fn some_question(rng: &mut Rng, watch: &Watch) -> Input {
    let call_id = watch.some_call(rng);
    let questions = (0..1 + rng.below(2))
        .map(|k| Question {
            header: None,
            question: format!("q{k}"),
            options: ["a", "b"]
                .iter()
                .map(|label| Choice {
                    label: label.to_string(),
                    description: None,
                })
                .collect(),
            multiple: rng.below(2) == 0,
        })
        .collect();
    Input::ToolAsks {
        at: at(49),
        call_id,
        questions,
    }
}

/// 一个回答：多半回答问着人的那几个，偶尔是对不上的调用；多半答得对得上，偶尔题数不对、选了
/// 没有的选项、一项选两次，也有只写了空白的。
pub(super) fn some_reply(rng: &mut Rng, watch: &Watch, next_id: &mut u64) -> Input {
    let asking: Vec<(CallId, usize)> = watch
        .questions
        .asking
        .iter()
        .map(|(call_id, questions)| (*call_id, questions.len()))
        .collect();
    let (call_id, count) = if rng.below(8) > 0 {
        asking[rng.below(asking.len() as u64) as usize]
    } else {
        (CallId::new(seq(1 + rng.below(watch.last())), 1).unwrap(), 1)
    };
    let count = if rng.below(10) == 0 { count + 1 } else { count };
    let answers = (0..count)
        .map(|_| {
            let picked = match rng.below(10) {
                0 => vec!["c"],
                1 => vec!["a", "a"],
                2..=4 => vec![],
                _ => vec!["a"],
            };
            Response {
                picked: picked.into_iter().map(str::to_string).collect(),
                text: (rng.below(4) == 0).then(|| " ".to_string()),
            }
        })
        .collect();
    reply(next_command(next_id), call_id, answers)
}
