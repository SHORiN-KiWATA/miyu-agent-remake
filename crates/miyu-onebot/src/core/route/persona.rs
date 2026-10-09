//! 判官带的人格的原文（施工 O-23 补，`onebot.md` 第一条「群里怎么叫她」第 12 条第 3 款，「施工时定的」第 102 到 104 条）：
//! 问判官的任务拿到群聊记录以后，经并着发的调用口读 `persona.read {persona, prompt: "persona"}`，读的是属主那几层叠好的、
//! 现在文件里的那一份。
//!
//! - 读到的记 `bridge.json` 的 `judge_persona_seconds`（出厂 60 秒），几个问判官的任务共用一份：这段时间里同一个人格不再读，
//!   人格改了判官最多晚这么久看到。没写人设的（`text` 是 `null` 或空的）也是读到了，照样记下，不带。
//! - 读不到的（核心拒了：人格删了、写错了、读出错）：这一次不带，运行日志记一行，照样问判官；不记下，下一次再读。
//!
//! 钟是 tokio 的：测试停住钟、拨着走，不用真等。

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use serde_json::json;
use tokio::time::Instant;

use crate::TARGET;
use crate::core::{Caller, Gone, reason};

/// 读哪一份提示词：人设（`protocol.md` 的 `persona.read`）。
const PROMPT: &str = "persona";

/// 记着的：人格编号 →（原文，读到的时刻）。原文是空的：这个人格没写人设。
type Held = HashMap<String, (Option<String>, Instant)>;

/// 读到的人格原文：可以复制，几个问判官的任务共用一份。
#[derive(Debug, Clone)]
pub(crate) struct Personas {
    /// 读到的记多久。
    keep: Duration,
    /// 记着的。
    held: Arc<Mutex<Held>>,
}

impl Personas {
    /// 一份空的，读到的记 `keep`。
    pub(crate) fn new(keep: Duration) -> Personas {
        Personas {
            keep,
            held: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 群会话 `session` 问判官要带的人格 `persona` 的说明：记着的、读到以后还不到 `keep` 的照用；别的经 `caller` 读，读到的
    /// 记下。没写人设的是空的；读不到的（核心拒了）也是空的，记一行运行日志（`session`、`persona`、原因码），不记下。
    ///
    /// # Errors
    ///
    /// 核心断开了。
    pub(super) async fn text(
        &self,
        persona: &str,
        session: &str,
        caller: &Caller,
    ) -> Result<Option<String>, Gone> {
        if let Some((text, _)) = self
            .lock()
            .get(persona)
            .filter(|(_, at)| at.elapsed() < self.keep)
        {
            return Ok(text.clone());
        }
        let params = json!({"persona": persona, "prompt": PROMPT});
        let reply = caller.call("persona.read", params).await?;
        if let Some(reason) = reason(&reply) {
            tracing::warn!(target: TARGET, session, persona, reason, "persona not read");
            return Ok(None);
        }
        let text = reply["result"]["text"]
            .as_str()
            .filter(|text| !text.is_empty())
            .map(str::to_string);
        let (now, keep) = (Instant::now(), self.keep);
        let mut held = self.lock();
        // 顺手扔掉过了的：人格删了、改名了的不会越攒越多。
        held.retain(|_, (_, at)| now.saturating_duration_since(*at) < keep);
        held.insert(persona.to_string(), (text.clone(), now));
        Ok(text)
    }

    /// 锁上记着的。锁坏了（拿着锁的地方 panic 了）照样用：里面只有读到的原文和时刻，坏不了（照 `core/caller.rs`）。
    fn lock(&self) -> MutexGuard<'_, Held> {
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
