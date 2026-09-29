//! 锚盖住的正好是锚那次请求加上它的回复（施工 6-1，`docs/blueprint/compaction.md`「怎么走」第一条）：用出厂的组装跑出来
//! 的真会话，每一次请求都照它发出时的有效历史挑一遍锚，查「请求只追加」这个前提在锚上成立。

use miyu_kernel::estimate::{self, Flat};
use miyu_kernel::event::Body;
use miyu_kernel::history::History;
use miyu_kernel::request::Message;
use miyu_kernel::testkit::Stage;

/// 一张图、一个文件都算 2000：出厂的那一种。
const FLAT: Flat = Flat {
    image: 2000,
    file: 2000,
};

/// 查每一次请求：有锚的，锚那次请求的消息原样是这一次的前几条（最后一条是人这边的，可以在后面多出几块），紧跟着的
/// 下一条是它的回复；算出的用量不少于供应商报的。
///
/// # Errors
///
/// 哪一次请求、哪一条不成立，写成一句话。成立的，交回有锚的请求有几次。
pub fn anchored(stage: &Stage) -> Result<usize, String> {
    let log = stage.log();
    let requests = stage.requests();
    let mut anchored = 0;
    for (index, (seen, request)) in requests.iter().enumerate() {
        let number = index + 1;
        let mut history = History::default();
        for event in log.iter().filter(|event| event.seq <= *seen) {
            history.append(event.clone());
        }
        let Some(anchor) = estimate::anchor(&history) else {
            continue;
        };
        let then_seen = log
            .iter()
            .find(|event| event.seq == anchor.seq)
            .and_then(|event| match &event.body {
                Body::ModelCalled(called) => Some(called.seen),
                _ => None,
            })
            .ok_or_else(|| {
                format!(
                    "第 {number} 次请求：锚 {} 不是 model.called",
                    anchor.seq.get()
                )
            })?;
        let (_, then) = requests[..index]
            .iter()
            .rev()
            .find(|(seen, _)| *seen == then_seen)
            .ok_or_else(|| format!("第 {number} 次请求：找不到锚那一次请求"))?;
        let n = then.messages.len();
        if u64::try_from(n).ok() != Some(anchor.messages) {
            return Err(format!(
                "第 {number} 次请求：锚记的消息条数和那一次请求对不上"
            ));
        }
        let Some((last, earlier)) = then.messages.split_last() else {
            return Err(format!("第 {number} 次请求：锚那一次请求一条消息都没有"));
        };
        if request.messages.get(..earlier.len()) != Some(earlier) {
            return Err(format!(
                "第 {number} 次请求：锚那一次的消息不是这一次的前几条"
            ));
        }
        let grown = match (last, request.messages.get(earlier.len())) {
            (then, Some(now)) if then == now => true,
            (Message::User { blocks: then }, Some(Message::User { blocks: now })) => {
                now.starts_with(then)
            }
            _ => false,
        };
        if !grown {
            return Err(format!("第 {number} 次请求：锚那一次的最后一条变了"));
        }
        if !matches!(request.messages.get(n), Some(Message::Assistant { .. })) {
            return Err(format!("第 {number} 次请求：锚盖住的第 {n} 条不是它的回复"));
        }
        if estimate::usage(request, Some(&anchor), &anchor.model, &FLAT) < anchor.reported {
            return Err(format!("第 {number} 次请求：算出的用量比供应商报的还少"));
        }
        anchored += 1;
    }
    Ok(anchored)
}
