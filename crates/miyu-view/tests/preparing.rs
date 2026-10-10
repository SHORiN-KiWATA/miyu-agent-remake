//! 后台提前压（施工 6-11 三补）：起压时状态里有 `preparing`，那一次说完了就没有；换上的那一条压缩带 `prepared`，翻页和视图流
//! 一样；在压时的 `doing` 带 `trigger`。数同内核的 `scenario/prepare.rs`：窗口 420，压缩线 400，过了 340 起压。

use std::sync::Arc;

use miyu_kernel::assemble::Assembler;
use miyu_kernel::block::{Block, Text};
use miyu_kernel::estimate::Flat;
use miyu_kernel::event::CompactTrigger;
use miyu_kernel::history::History;
use miyu_kernel::id::Seq;
use miyu_kernel::request::{Message, Request};
use miyu_kernel::session::{Compaction, Policy};
use miyu_kernel::testkit::{Line, Stage, Streamed};
use miyu_view::{Doing, Projector, Status};

use crate::support::*;

/// 组装：有效历史一条事件一行（同内核场景测试的替身），用量才随历史长；摘要请求最后一行是 `summarize`。
struct Lines;

impl Assembler for Lines {
    fn assemble(&self, history: &History) -> Request {
        let messages = history
            .events()
            .iter()
            .map(|event| Message::User {
                blocks: vec![Block::Text(Text {
                    text: format!("{} {}", event.seq, event.body.kind()),
                })],
            })
            .collect();
        Request {
            tools: Vec::new(),
            system: "lines".to_string(),
            messages,
            stable: 0,
            continuation: false,
            described: Default::default(),
            output_cap: None,
        }
    }

    fn summarize(&self, history: &History, upto: Seq, _: Option<Seq>, _: Option<&str>) -> Request {
        let mut request = self.assemble(&history.until(upto));
        request.messages.push(Message::User {
            blocks: vec![Block::Text(Text {
                text: "summarize".to_string(),
            })],
        });
        request
    }

    fn summarize_isolated(
        &self,
        history: &History,
        upto: Seq,
        cut: Option<Seq>,
        instructions: Option<&str>,
    ) -> Request {
        self.summarize(history, upto, cut, instructions)
    }

    fn summary(&self, reply: &[Block]) -> Option<String> {
        reply.iter().find_map(|block| match block {
            Block::Text(text) => Some(text.text.clone()),
            _ => None,
        })
    }
}

/// 会提前压的策略：一条事件一行地组装，输出预留、余量各 10，尾巴 40，提前量 60。
fn preparing_policy() -> Policy {
    let mut policy = policy();
    policy.assembler = Box::new(Lines);
    policy.compaction = Some(Compaction {
        reserve_cap: 10,
        margin: 10,
        line_percent: 100,
        margin_percent: 100,
        tail: 40,
        lead: 60,
        price: Flat {
            image: 50,
            file: 50,
        },
        rebuild: None,
        pause: None,
        shorten: None,
        isolate: true,
    });
    policy
}

fn words(tokens: usize) -> String {
    "abcd".repeat(tokens)
}

/// 两轮：第二轮的请求过了起压线，提前压的那一次在路上（`held` 的等放）。
fn two_turns(held: bool) -> Stage {
    let mut stage = policy_stage(preparing_policy);
    stage.prepare(true);
    stage.limits(Some(420), None);
    let line = Line::says("P1");
    stage.prepare_model([if held { line.held() } else { line }]);
    stage.model([
        Line::says(&words(50)).reports(330),
        Line::says(&words(10)).reports(390),
    ]);
    stage.say(&words(50));
    stage.say(&words(10));
    stage
}

/// 落了盘的和瞬时的照先后喂，每喂一条记一份状态。
fn statuses(stage: &Stage) -> Vec<Status> {
    let mut projector = Projector::new(Arc::new(texts("en")), None);
    stage
        .stream()
        .iter()
        .map(|item| {
            match item {
                Streamed::Event(event) => drop(projector.event(event)),
                Streamed::Transient(transient) => drop(projector.transient(transient)),
            }
            projector.status()
        })
        .collect()
}

#[test]
fn preparing_shows_from_the_start_until_that_request_ends() {
    let mut stage = two_turns(true);
    let live = statuses(&stage);
    let preparing = live.last().and_then(|status| status.preparing);
    assert_eq!(preparing.map(|p| p.seen), Some(8), "起压了：{live:#?}");
    stage.release_prepare();
    let live = statuses(&stage);
    assert_eq!(
        live.last().and_then(|status| status.preparing),
        None,
        "说完了"
    );
}

#[test]
fn the_swapped_compaction_is_prepared_in_a_page_too() {
    let mut stage = two_turns(false);
    stage.model([Line::says("好")]);
    stage.say(&words(10));
    let entries = same(&stage);
    let compactions = of_kind(&entries, "notice")
        .into_iter()
        .map(json)
        .filter(|notice| notice["what"] == "compaction")
        .collect::<Vec<_>>();
    assert_eq!(compactions.len(), 1, "{entries:#?}");
    assert_eq!(compactions[0]["prepared"], true, "翻页也带着");
    assert_eq!(compactions[0]["trigger"], "auto");
}

#[test]
fn compacting_on_the_spot_carries_its_trigger() {
    let mut stage = two_turns(true);
    // 第三轮放不下了，在等在路上的那一次：推进度，`doing` 带 auto。
    stage.model([Line::says("好").reports(400)]);
    stage.say(&words(10));
    stage.model([Line::says("又好")]);
    stage.say(&words(10));
    let live = statuses(&stage);
    let doing = live.iter().find_map(|status| match &status.doing {
        Some(Doing::Compacting { trigger, .. }) => Some(trigger.clone()),
        _ => None,
    });
    assert_eq!(doing, Some(CompactTrigger::Auto), "{live:#?}");
}
