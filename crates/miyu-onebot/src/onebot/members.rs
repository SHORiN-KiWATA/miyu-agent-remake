//! 群成员的名字（`onebot.md` 第一条「群消息」第 5 条，施工 O-22）：正文里的 @ 写成 `@名字`，名字照这里的缓存。NapCat 的 `at`
//! 段只有号，名字得另取（「施工时定的」第 59 条）：每条群消息的发的人顺手记下，缓存里没有的经 `get_group_member_info` 问。
//!
//! 按群、按号记，记多久照 `bridge.json` 的 `member_names_seconds`；过了的不用。时刻由用的一方交进来，测试不用等。
//!
//! 施工 O-31 另记身份（群主、管理员、普通成员，「平台工具（一）」第 7 条）：禁言不动群主和管理员。群消息的 `sender.role`、
//! `get_group_member_info` 回的 `role` 都带，同一个时限（「施工时定的」第 184 条）。

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

/// 问一个群成员的动作。
pub const MEMBER_INFO: &str = "get_group_member_info";

/// 一个人在群里的身份（OneBot 的 `role`，施工 O-31）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rank {
    /// 群主：`owner`。
    Owner,
    /// 群管理员：`admin`。
    Admin,
    /// 普通成员：`member`。
    Member,
}

/// 群成员的名字缓存。
#[derive(Debug)]
pub struct Members {
    /// 记多久。
    keep: Duration,
    /// （群号，号）→（名字，记下的时刻）。
    names: HashMap<(i64, i64), (String, Instant)>,
    /// （群号，号）→（身份，记下的时刻）（施工 O-31）。
    ranks: HashMap<(i64, i64), (Rank, Instant)>,
}

impl Members {
    /// 一个空的缓存，名字记 `keep`。
    pub fn new(keep: Duration) -> Members {
        Members {
            keep,
            names: HashMap::new(),
            ranks: HashMap::new(),
        }
    }

    /// 群 `group` 里号是 `user` 的人此刻 `now` 叫 `name`：记下（盖掉原来的），顺手扔掉过了的，缓存不会越攒越多。
    pub fn remember(&mut self, group: i64, user: i64, name: String, now: Instant) {
        let keep = self.keep;
        self.names
            .retain(|_, (_, at)| now.saturating_duration_since(*at) < keep);
        self.names.insert((group, user), (name, now));
    }

    /// 群 `group` 里号是 `user` 的人叫什么：记下以后到此刻 `now` 还不到 `keep` 的才有。
    pub fn name(&self, group: i64, user: i64, now: Instant) -> Option<&str> {
        self.names
            .get(&(group, user))
            .filter(|(_, at)| now.saturating_duration_since(*at) < self.keep)
            .map(|(name, _)| name.as_str())
    }

    /// 群 `group` 里号是 `user` 的人此刻 `now` 的身份是 `rank`（施工 O-31）：记下（盖掉原来的），顺手扔掉过了的。
    pub fn ranked(&mut self, group: i64, user: i64, rank: Rank, now: Instant) {
        let keep = self.keep;
        self.ranks
            .retain(|_, (_, at)| now.saturating_duration_since(*at) < keep);
        self.ranks.insert((group, user), (rank, now));
    }

    /// 群 `group` 里号是 `user` 的人的身份：同 [`Members::name`]，过了的不用。
    pub fn rank(&self, group: i64, user: i64, now: Instant) -> Option<Rank> {
        self.ranks
            .get(&(group, user))
            .filter(|(_, at)| now.saturating_duration_since(*at) < self.keep)
            .map(|(rank, _)| *rank)
    }
}

/// `get_group_member_info` 的参数：群 `group` 里号是 `user` 的人，让 NapCat 用它自己的缓存（`no_cache: false`）。
pub fn member_info(group: i64, user: i64) -> Value {
    json!({"group_id": group, "user_id": user, "no_cache": false})
}

/// 一个人在群里叫什么：`who` 是群消息的 `sender`，或者 `get_group_member_info` 回的 `data`。`card`（群名片）去掉首尾
/// 空白不空的照它，不然 `nickname`（昵称，同样不空的）；都没有的是空的。照原样，没洗。
pub fn display_name(who: &Value) -> Option<String> {
    ["card", "nickname"]
        .iter()
        .filter_map(|key| who[*key].as_str())
        .find(|name| !name.trim().is_empty())
        .map(str::to_string)
}

/// 一个人在群里的身份（施工 O-31）：`who` 是群消息的 `sender`，或者 `get_group_member_info` 回的 `data`；`role` 是 `owner`、
/// `admin`、`member` 之一的才认，别的、没有的是空的。
pub fn rank_of(who: &Value) -> Option<Rank> {
    match who["role"].as_str()? {
        "owner" => Some(Rank::Owner),
        "admin" => Some(Rank::Admin),
        "member" => Some(Rank::Member),
        _ => None,
    }
}
