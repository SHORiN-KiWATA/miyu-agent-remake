//! `resources/software/onebot/bridge.json`（`onebot.md`「施工时定的」第 15 条）：桥自己的数放在它的软件包里，不进配置
//! 清单（照网页软件的 `web.json`，`web-module.md`「起草时定的」第 25 条）。起来时读一次。
//!
//! 是数据，不发给模型，不进登记簿（`xtask/src/ledger.rs` 只豁免这一份文件：这个包以后要放给模型看的字）。

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use serde::Deserialize;

/// 文件在资源目录里的位置。
pub const FILE: &str = "software/onebot/bridge.json";

/// 读好的一份。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tuning {
    /// NapCat 连得进来的路径（第 2 条），别的 404。
    pub paths: Vec<String>,
    /// 一次 OneBot 调用等回应最多几秒（第 4 条）。
    pub call_timeout_seconds: u64,
    /// 往一条 NapCat 的连接写，最多攒几帧没写出去；满了，写的一方等着。至少 1。
    pub write_queue: usize,
    /// 读出来的消息、撤回（施工 O-22 起群的也算）最多攒几条没交给跟核心的那一头；满了，读 NapCat 的那一头等着。至少 1。
    pub inbound_queue: usize,
    /// 接不了 TCP 连接（打开的文件太多这类）时，歇几毫秒再接，不空转（照核心的规矩）。
    pub accept_retry_millis: u64,
    /// 跟核心握手，最多等几秒回应（第 1 条，施工 O-18）：从终端跑起来的等不到就退，说 `serve` 只由核心拉起。
    pub hello_seconds: u64,
    /// `logs -f` 隔几毫秒看一次运行日志长了没有（施工 O-18）。
    pub follow_millis: u64,
    /// 要用场所规则时，隔几毫秒才看一眼系统的两处变没变（施工 O-21，`onebot.md`「场所规则和出厂数据」第 3 条）。
    pub rules_check_millis: u64,
    /// 群成员的名字记几秒（施工 O-22，`onebot.md`「群消息」第 5 条）。
    pub member_names_seconds: u64,
    /// 判官全局最多同时问几个（施工 O-23 下，`onebot.md`「群里怎么叫她」第 12 条；18 第七节）。至少 1：是桥这个进程的，不按
    /// 场所改，所以不在群聊内核的参数里（`chat.md` 第八条施工时定的第 5 条）。
    pub judge_concurrency: usize,
    /// 名额满了，问判官的排队最多等几秒（同上），等不到的当判不了。
    pub judge_queue_seconds: u64,
    /// 判官带的人格原文读到以后记几秒，这段时间里同一个人格不再读（施工 O-23 补，`onebot.md`「群里怎么叫她」第 12 条第 3 款，
    /// 「施工时定的」第 103 条）。
    pub judge_persona_seconds: u64,
    /// 群里的命令回执发出去几秒后撤回（施工 O-25 上，`onebot.md`「斜杠命令」第 7 条；18 第十节）。0 是回了编号就撤。
    pub receipt_recall_seconds: u64,
    /// 出站排着的（她被禁言、机器人号没连着）入队以后过几秒还没交出去的作废（施工 O-25 中，`onebot.md`「出站队列」第 5 条）。
    /// 至少 1：0 的话入队的时刻就到了期限，一条都发不出去（「施工时定的」第 124 条）。
    pub queue_expire_seconds: u64,
    /// 群里判过要回的那一条上贴哪个表情（施工 O-25 下，`onebot.md`「贴表情」第 2 条；18 第七节）：QQ 表情的编号，写成字，原样交
    /// `set_msg_emoji_like` 的 `emoji_id`。
    pub reaction_emoji: String,
    /// 贴了以后过几秒她还没回、这一轮还没完的，摘掉（施工 O-25 下，「贴表情」第 3 条）。0 是贴了就摘（「施工时定的」第 135 条）。
    pub reaction_seconds: u64,
    /// WebUI 的数（施工 O-16，`onebot.md` 第二条）。
    pub web: WebTuning,
}

/// WebUI 的数：页面的内容安全策略、类型表（照网页软件的 `web.json`），`/status` 验过的登录令牌记多久。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebTuning {
    /// 页面的 `Content-Security-Policy`。
    pub csp: String,
    /// 扩展名（小写）→ 媒体类型；表里没有的给 `application/octet-stream`。
    pub types: BTreeMap<String, String>,
    /// `/status` 验过的登录令牌记几秒，这几秒里不再和核心握手（「施工时定的」第 3 条）。
    pub status_cache_seconds: u64,
}

impl WebTuning {
    /// 验过的登录令牌记多久。
    pub fn status_cache(&self) -> Duration {
        Duration::from_secs(self.status_cache_seconds)
    }
}

impl Tuning {
    /// 读资源目录 `resources` 里的 [`FILE`]。
    ///
    /// # Errors
    ///
    /// 读不了、不是这个形状、队列写了 0（建不了队列）、判官的并发写了 0（一个都问不了）、排着的过期写了 0（一条都发不出去）：
    /// 原话里说是哪个文件。
    pub fn load(resources: &Path) -> Result<Tuning, String> {
        let path = resources.join(FILE);
        let bad = |why: String| format!("{} not readable: {why}", path.display());
        let text = std::fs::read_to_string(&path).map_err(|error| bad(error.to_string()))?;
        let tuning: Tuning = serde_json::from_str(&text).map_err(|error| bad(error.to_string()))?;
        if tuning.write_queue == 0
            || tuning.inbound_queue == 0
            || tuning.judge_concurrency == 0
            || tuning.queue_expire_seconds == 0
        {
            return Err(bad("write_queue, inbound_queue, judge_concurrency and queue_expire_seconds must be at least 1".to_string()));
        }
        Ok(tuning)
    }

    /// 一次调用等回应最多多久。
    pub fn call_timeout(&self) -> Duration {
        Duration::from_secs(self.call_timeout_seconds)
    }

    /// 接不了连接时歇多久。
    pub fn accept_retry(&self) -> Duration {
        Duration::from_millis(self.accept_retry_millis)
    }

    /// 跟核心握手最多等多久。
    pub fn hello(&self) -> Duration {
        Duration::from_secs(self.hello_seconds)
    }

    /// `logs -f` 隔多久看一次。
    pub fn follow(&self) -> Duration {
        Duration::from_millis(self.follow_millis)
    }

    /// 隔多久才看一眼系统的场所规则变没变。
    pub fn rules_check(&self) -> Duration {
        Duration::from_millis(self.rules_check_millis)
    }

    /// 群成员的名字记多久。
    pub fn member_names(&self) -> Duration {
        Duration::from_secs(self.member_names_seconds)
    }

    /// 问判官的排队最多等多久。
    pub fn judge_queue(&self) -> Duration {
        Duration::from_secs(self.judge_queue_seconds)
    }

    /// 判官带的人格原文记多久。
    pub fn judge_persona(&self) -> Duration {
        Duration::from_secs(self.judge_persona_seconds)
    }

    /// 群里的命令回执发出去多久以后撤回。
    pub fn receipt_recall(&self) -> Duration {
        Duration::from_secs(self.receipt_recall_seconds)
    }

    /// 出站排着的多久过期。
    pub fn queue_expire(&self) -> Duration {
        Duration::from_secs(self.queue_expire_seconds)
    }

    /// 贴的表情过多久摘。
    pub fn reaction(&self) -> Duration {
        Duration::from_secs(self.reaction_seconds)
    }
}
