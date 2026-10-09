//! 抽取（施工 R-6 上，`docs/blueprint/memory.md` 第六条）：会话闲下来以后，把上次抽到以后的这一段里值得记的抽出来，记成她的
//! 记忆。这里是会话那一层的几样：给模型看的字、一个会话的抽取状态、照这一段拼请求、派出去的那一件活。什么时候起、这一段
//! 从哪来在 `actor/extract.rs`。
//!
//! - **这一段**：会话那一层照日志拼的历史（留着一切的那一份），交给会话的组装器渲染（`Session::spoken_in`），所以压缩过的也
//!   在。人的话接在它触发的那一轮上；还没答的那一句不算进来，等下次。
//! - **拼法**（单发）：指令在前，后面是这一段的几轮，一轮一块（编号、日期、人的话、她的回答），一条 user，不带 system、
//!   工具面。最多 [`ROOM`] 字节：放不下的照先后取最老的几轮，`upto` 记到取到的最后一轮；一轮自己就放不下的留头尾、截中间，
//!   照样抽，`upto` 照样往前走，不卡在一轮上。
//! - **跳过**：这一段里她调过 `remember`、`forget` 的整段跳过，只记抽到了哪。
//! - **发**：经一次性入口发 `memory.organizer`（没写的照 `models.chat`），用途 `memory`，记在会话属主的账上。发出去以前遮掉
//!   key（配置里引用的密钥的原文、常见的写法），交回来的再遮一遍。
//! - **出错**：这一段不往前挪，下次闲了再抽；同一段连着三次不成的跳过，记 `WARN memory extraction failed`。

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use miyu_config::Value;
use miyu_config::secret::Reference;
use miyu_kernel::assemble::Spoken;
use miyu_kernel::block::{Block, Text};
use miyu_kernel::id::{AccountId, Seq, SessionId, TurnId};
use miyu_kernel::request::Message;
use miyu_kernel::template::Template;
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_recall::Skipped;
use miyu_recall::extract::candidates;
use miyu_recall::redact::{KeyShapes, redact};
use miyu_store::blob::Blobs;
use miyu_tool::load::{self, LoadError, say};
use miyu_tool::{FORGET, REMEMBER, TEXT_CHARS};

use super::Keeper;
use crate::TARGET;
use crate::blocking::blocking;
use crate::clock::wall_now;
use crate::config::{Turn, TurnConfig};
use crate::route::{Ask, OneShot};

/// 这一段最多几个字节：和回顾的对话记录一个量级（约 8000 token）。
pub(crate) const ROOM: usize = 32 * 1024;

/// 一轮和一轮之间。
const GAP: &str = "\n\n";

/// 用途：记运行日志、记账，key 照它钉。
const PURPOSE: &str = "memory";

/// 同一段连着抽不成几次就跳过。
const TRIES: u32 = 3;

/// 记忆这个软件包在资源目录里的名字（和 `summary.rs` 的同一个）。
const PACKAGE: &str = "memory";

/// 抽取给模型看的字（`resources/software/memory/extract/`，登记簿）：指令、一轮的头尾、人的话、她的回答、截了中间的那一句。
/// 换进去的字照模板的规矩转义成一行（`miyu_kernel::template`）：人的话伪造不了一轮的尾巴。
#[derive(Debug, Clone)]
pub struct ExtractTexts {
    instruction: Template,
    open: Template,
    close: Template,
    user: Template,
    assistant: Template,
    excerpted: Template,
}

impl ExtractTexts {
    /// 从资源目录 `resources` 读：`extract/instruction.txt`、`turn-open.txt`（`{turn}`、`{date}`）、`turn-close.txt`、
    /// `user.txt`、`assistant.txt`（`{text}`）、`excerpted.txt`。
    ///
    /// # Errors
    ///
    /// 哪一份读不出来、写法不对、要了别的字段。
    pub fn load(resources: &Path) -> Result<ExtractTexts, LoadError> {
        let text =
            |name: &str, fields: &[&str]| load::text(resources, PACKAGE, "extract", name, fields);
        Ok(ExtractTexts {
            instruction: text("instruction", &[])?,
            open: text("turn-open", &["turn", "date"])?,
            close: text("turn-close", &[])?,
            user: text("user", &["text"])?,
            assistant: text("assistant", &["text"])?,
            excerpted: text("excerpted", &[])?,
        })
    }
}

/// 抽取要的几样：核心起来时交给记忆一次（[`super::Memory::give_extraction`]）；没交的不抽。
#[derive(Clone)]
pub struct Extraction {
    /// 给模型看的字。
    pub texts: ExtractTexts,
    /// 常见的 key 写法（`resources/core/memory/secrets.toml`）。
    pub shapes: KeyShapes,
    /// 一次性入口：照 `memory.organizer` 发。
    pub ask: OneShot,
    /// 编码要的 blob：抽取的请求只有字，用不上，一次性入口要它。
    pub blobs: Blobs,
    /// 闲多久才抽：没有的照配置 `memory.extract_idle`；测试里设短的，不用真等一分钟。
    pub idle: Option<std::time::Duration>,
}

impl std::fmt::Debug for Extraction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Extraction")
            .field("shapes", &self.shapes)
            .finish_non_exhaustive()
    }
}

/// 一个会话的抽取：闹钟是第几个、有没有一次在路上、同一段失败了几次。
#[derive(Debug, Default)]
pub(crate) struct Extractor {
    state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
    /// 第几个闹钟：上了新的、撤掉的都往上数，响的时候对不上的是作废的。
    generation: u64,
    /// 有一次在路上。
    running: bool,
    /// 从第几条起抽、失败了几次。
    failures: Option<(Seq, u32)>,
}

impl Extractor {
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 上一个新闹钟，交回它是第几个。
    pub(crate) fn arm(&self) -> u64 {
        let mut state = self.state();
        state.generation += 1;
        state.generation
    }

    /// 撤掉闹钟：新一轮开始了。在路上的那一次照样走完。
    pub(crate) fn cancel(&self) {
        self.state().generation += 1;
    }

    /// 第 `generation` 个闹钟还算数、没有一次在路上。
    pub(crate) fn due(&self, generation: u64) -> bool {
        let state = self.state();
        state.generation == generation && !state.running
    }

    /// 第 `generation` 个闹钟还算数就开始一次，交回开没开成。
    pub(crate) fn start(&self, generation: u64) -> bool {
        let mut state = self.state();
        if state.generation != generation || state.running {
            return false;
        }
        state.running = true;
        true
    }

    /// 这一次读不成（日志读不了）：放下，不算这一段失败了一次，下次闲了再来。
    pub(crate) fn abandon(&self) {
        self.state().running = false;
    }

    /// 这一次走完了：成了的清掉失败的次数；没成的交回从第 `after` 条起这是第几次。
    fn finish(&self, after: Seq, ok: bool) -> u32 {
        let mut state = self.state();
        state.running = false;
        if ok {
            state.failures = None;
            return 0;
        }
        let tries = match state.failures {
            Some((from, tries)) if from == after => tries + 1,
            _ => 1,
        };
        state.failures = (tries < TRIES).then_some((after, tries));
        tries
    }
}

/// 一轮：编号、她回答的时刻、人这边的几句、她的回答、她那一条的序号。
struct Exchange {
    turn: TurnId,
    at: Timestamp,
    said: Vec<String>,
    answer: String,
    seq: Seq,
}

/// 把几段话接成几轮：人的话接在它后面那一轮她的回答上；最后还没答的那几句不算。
fn exchanges(spoken: &[Spoken]) -> Vec<Exchange> {
    let mut said = Vec::new();
    let mut exchanges = Vec::new();
    for one in spoken {
        match (one.assistant, one.turn) {
            (true, Some(turn)) => exchanges.push(Exchange {
                turn,
                at: one.at,
                said: std::mem::take(&mut said),
                answer: one.text.clone(),
                seq: one.seq,
            }),
            (true, None) => {}
            (false, _) => said.push(one.text.clone()),
        }
    }
    exchanges
}

/// 这一段怎么办。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Plan {
    /// 答了的轮数不够：等下次。
    Wait,
    /// 整段跳过（她当场记过了），只记抽到第 `upto` 条。
    Skip {
        /// 抽到第几条。
        upto: Seq,
    },
    /// 发出去抽。
    Ask {
        /// 抽到第几条：取到的最后一轮她那一条。
        upto: Seq,
        /// 请求的字：指令接这一段。
        text: String,
        /// 这一段里的几轮的编号：交回的候选只认这几轮。
        turns: BTreeSet<u64>,
        /// 放不下、还剩几轮没取：抽完接着抽剩下的。
        more: bool,
    },
}

/// 照这一段 `spoken`（她在里面调过的工具 `called`）定怎么办：答了的轮数至少 `min_turns`；日期照会话的时区 `offset`。
pub(crate) fn plan(
    spoken: &[Spoken],
    called: &BTreeSet<String>,
    min_turns: usize,
    offset: UtcOffset,
    texts: &ExtractTexts,
) -> Plan {
    let exchanges = exchanges(spoken);
    let Some(last) = exchanges.last().map(|exchange| exchange.seq) else {
        return Plan::Wait;
    };
    if exchanges.len() < min_turns.max(1) {
        return Plan::Wait;
    }
    if called.contains(REMEMBER) || called.contains(FORGET) {
        return Plan::Skip { upto: last };
    }
    let mut blocks: Vec<String> = Vec::new();
    let mut turns = BTreeSet::new();
    let mut upto = last;
    let mut bytes = 0;
    for exchange in &exchanges {
        let block = render(exchange, offset, texts);
        let needed = block.len() + if blocks.is_empty() { 0 } else { GAP.len() };
        if !blocks.is_empty() && bytes + needed > ROOM {
            break;
        }
        // 一轮自己就放不下的截中间也抽：不卡在一轮上。
        let block = match block.len() > ROOM {
            true => excerpt(&block, ROOM, texts),
            false => block,
        };
        bytes += needed;
        blocks.push(block);
        turns.insert(exchange.turn.started().get());
        upto = exchange.seq;
        if bytes >= ROOM {
            break;
        }
    }
    let instruction = say(&texts.instruction, &[]);
    Plan::Ask {
        upto,
        text: format!("{}\n{}", instruction.trim_end(), blocks.join(GAP)),
        turns,
        more: upto != last,
    }
}

/// 一轮写成一块：头、人的话、她的回答、尾，一样一行。
fn render(exchange: &Exchange, offset: UtcOffset, texts: &ExtractTexts) -> String {
    let mut lines = vec![say(
        &texts.open,
        &[
            ("turn", &exchange.turn.started().get().to_string()),
            ("date", &exchange.at.local_date(offset)),
        ],
    )];
    lines.extend(
        exchange
            .said
            .iter()
            .map(|text| say(&texts.user, &[("text", text)])),
    );
    lines.push(say(&texts.assistant, &[("text", &exchange.answer)]));
    lines.push(say(&texts.close, &[]));
    lines.join("\n")
}

/// 放不下的一块留头尾、截中间，至多 `room` 字节左右；截在字的边界上。
fn excerpt(block: &str, room: usize, texts: &ExtractTexts) -> String {
    let marker = say(&texts.excerpted, &[]);
    let half = room.saturating_sub(marker.len()) / 2;
    let head_end = (0..=half)
        .rev()
        .find(|at| block.is_char_boundary(*at))
        .unwrap_or(0);
    let tail_start = (block.len().saturating_sub(half)..=block.len())
        .find(|at| block.is_char_boundary(*at))
        .unwrap_or(block.len());
    format!("{}{marker}{}", &block[..head_end], &block[tail_start..])
}

/// 配置里引用的密钥的原文（照这一份配置取得到的）：发出去以前、记下以前遮掉。
fn secrets(config: &Turn) -> Vec<String> {
    let values = config.resolved.values();
    let mut found = Vec::new();
    for key in values.keys() {
        let references: Vec<&Reference> = match values.get(key) {
            Some(Value::Secret(reference)) => vec![reference],
            Some(Value::List(list)) => list
                .iter()
                .filter_map(|value| match value {
                    Value::Secret(reference) => Some(reference),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        found.extend(
            references
                .into_iter()
                .filter_map(|reference| config.secret(reference))
                .map(|secret| secret.expose().to_string()),
        );
    }
    found
}

/// 派出去抽的一次。
pub(crate) struct Job {
    /// 记在哪一间、听众是谁。
    pub(crate) keeper: Keeper,
    /// 字、key 的写法、一次性入口。
    pub(crate) extraction: Extraction,
    /// 这一轮的配置：照它取 key、发。
    pub(crate) config: TurnConfig,
    /// 记在谁的账上：会话的属主。
    pub(crate) owner: AccountId,
    /// 哪个会话。
    pub(crate) session: SessionId,
    /// 从第几条以后抽（上次抽到的；没抽过的是 0 那种，照 `Seq` 最小的）。
    pub(crate) after: Seq,
    /// 这一回的状态：走完了清掉。
    pub(crate) extractor: Arc<Extractor>,
    /// 放不下、还剩几轮的，抽完了照它马上再起一次（不用再等闲）。
    pub(crate) again: Box<dyn Fn() + Send + Sync>,
}

impl Job {
    /// 照 `plan` 走一次：跳过的只记抽到了哪；要发的发、读、遮、记。
    pub(crate) async fn run(self, plan: Plan) {
        match plan {
            Plan::Wait => {
                self.extractor.finish(self.after, true);
            }
            Plan::Skip { upto } => {
                self.mark(upto, Vec::new(), Some(Skipped::Remembered)).await;
                self.extractor.finish(self.after, true);
            }
            Plan::Ask {
                upto,
                text,
                turns,
                more,
            } => {
                if self.ask(upto, text, turns).await && more {
                    (self.again)();
                }
            }
        }
    }

    /// 发、读、遮、记；交回记成了没有。
    async fn ask(&self, upto: Seq, text: String, turns: BTreeSet<u64>) -> bool {
        let started = Instant::now();
        let secrets = secrets(&self.config);
        let shapes = self.extraction.shapes.clone();
        let organizer =
            crate::settings::MemorySettings::from(&self.config.resolved.values()).organizer;
        tracing::info!(target: TARGET, session = %self.session, turns = turns.len(), "memory extraction started");
        let ask = Ask {
            model: organizer,
            purpose: PURPOSE.to_string(),
            system: String::new(),
            messages: vec![Message::User {
                blocks: vec![Block::Text(Text {
                    text: redact(&text, &secrets, &shapes),
                })],
            }],
            max_tokens: None,
            owner: self.owner.clone(),
        };
        let answered = self
            .extraction
            .ask
            .call(&self.config, &self.extraction.blobs, ask)
            .await
            .map_err(|unanswered| unanswered.reason().to_string())
            .and_then(|answer| candidates(&answer.text, &turns, TEXT_CHARS));
        match answered {
            Ok(mut found) => {
                for candidate in &mut found {
                    candidate.text = redact(&candidate.text, &secrets, &shapes);
                }
                let recorded = self.mark(upto, found, None).await;
                if let Some(count) = recorded {
                    let took_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                    tracing::info!(target: TARGET, session = %self.session, count, took_ms, "memory extracted");
                }
                self.extractor.finish(self.after, true);
                recorded.is_some()
            }
            Err(why) => {
                let tries = self.extractor.finish(self.after, false);
                tracing::warn!(target: TARGET, session = %self.session, tries, error = why.as_str(), "memory extraction failed");
                if tries >= TRIES {
                    self.mark(upto, Vec::new(), Some(Skipped::Failed)).await;
                }
                false
            }
        }
    }

    /// 记下几条、记抽到了第 `upto` 条，交回记下几条；写不进的记一行，交回没有。
    async fn mark(
        &self,
        upto: Seq,
        found: Vec<miyu_recall::extract::Candidate>,
        skipped: Option<Skipped>,
    ) -> Option<u32> {
        let (keeper, session) = (self.keeper.clone(), self.session.clone());
        let recorded =
            blocking(move || keeper.record_extraction(wall_now(), &session, upto, found, skipped))
                .await;
        recorded
            .inspect_err(|error| {
                tracing::warn!(target: TARGET, session = %self.session, error = ?error, "memory extraction not recorded");
            })
            .ok()
    }
}

#[cfg(test)]
mod tests;
