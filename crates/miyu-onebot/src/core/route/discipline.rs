//! 线路规程（施工 O-23 下，`onebot.md` 第一条「群里怎么叫她」第 10 条；18 第七节那张表）：场所规则的 `discipline` 决定一条
//! 群消息的哪些条件算数、看不看顶替、走哪条路。纯逻辑。
//!
//! | 线路规程 | 算数的条件 | 顶替 | 走哪条路 |
//! |---|---|---|---|
//! | `chatty` | 全部（额度满了去掉抽样） | 看 | 群聊内核的 `route` |
//! | `when-called` | 冲她来、续聊、违规旗 | 看 | 只有违规旗的判官只查违规，别的有条件的开一轮 |
//! | `wake` | 冲她来、续聊、违规旗 | 不看 | 同 `when-called` |
//! | `every-message` | 去掉抽样（违规旗只记进判断） | 不看 | 有字的开一轮 |
//!
//! 不是 `chatty` 的不抽样、不打分。违规关键词只能把判官拉起来，不能直接定违规（18 第七节「违规审核」）：`when-called`、`wake`
//! 里只有违规旗的照 `chatty` 的只查违规问判官，严重程度够了才回；`every-message` 有字的本来就回，违规旗不改它（「施工时定的」
//! 第 90 条）。写在桥里、不进群聊内核（第 87 条）：别的平台的桥要用时再挪。

use miyu_chat::{Conditions, Kind, Route, Standing, route};

/// 一个群的线路规程。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Discipline {
    /// 看情况插话：主动回复判断那一整套，问判官。
    Chatty,
    /// 叫她才回：冲她来、续聊、同一个人接连发的几条。
    WhenCalled,
    /// 唤醒：听到唤醒词（触发词）才回；回复以后一小段时间内不用再叫，照续聊算（「施工时定的」第 89 条）。
    Wake,
    /// 每条都回：有字的都开一轮。
    EveryMessage,
}

impl Discipline {
    /// 场所规则里的写法 `text`：没设的照 `chatty`（「施工时定的」第 88 条）。规则的选项群聊内核查过，认不出的只有手造的，
    /// 也照 `chatty`。
    pub(super) fn read(text: Option<&str>) -> Discipline {
        match text {
            Some("when-called") => Discipline::WhenCalled,
            Some("wake") => Discipline::Wake,
            Some("every-message") => Discipline::EveryMessage,
            _ => Discipline::Chatty,
        }
    }

    /// 写进判断的名字：和场所规则里的写法一样。
    pub(super) fn name(self) -> &'static str {
        match self {
            Discipline::Chatty => "chatty",
            Discipline::WhenCalled => "when-called",
            Discipline::Wake => "wake",
            Discipline::EveryMessage => "every-message",
        }
    }

    /// 看不看顶替：`chatty`、`when-called` 看，18 第七节那张表只给它俩写了「同一个人接连发的几条」。
    pub(super) fn supersedes(self) -> bool {
        matches!(self, Discipline::Chatty | Discipline::WhenCalled)
    }

    /// 留下算数的条件，照原来的先后。`rate_full` 是额度满了：`chatty` 这段时间不抽样（18 第六节）；别的本来就不抽样。
    pub(super) fn keep(self, found: Conditions, rate_full: bool) -> Conditions {
        let kept = |kind: Kind| match self {
            Discipline::Chatty => kind != Kind::Probability || !rate_full,
            Discipline::WhenCalled | Discipline::Wake => {
                matches!(kind, Kind::Direct | Kind::Continuation | Kind::Moderation)
            }
            Discipline::EveryMessage => kind != Kind::Probability,
        };
        Conditions {
            hits: found
                .hits
                .into_iter()
                .filter(|hit| kept(hit.kind))
                .collect(),
        }
    }

    /// 走哪条路：`chatty` 照群聊内核的 `route`；`when-called`、`wake` 只有违规旗的判官只查违规，别的有条件的开一轮，没有的只记下；
    /// `every-message` 有字的开一轮，只有带的东西（`media_only`）的只记下。
    pub(super) fn route(
        self,
        conditions: &Conditions,
        standing: Standing,
        media_only: bool,
    ) -> Route {
        let commit_if = |yes: bool| if yes { Route::Commit } else { Route::Record };
        match self {
            Discipline::Chatty => route(conditions, standing),
            Discipline::WhenCalled | Discipline::Wake if only_flagged(conditions) => {
                Route::ModerationOnly
            }
            Discipline::WhenCalled | Discipline::Wake => commit_if(!conditions.hits.is_empty()),
            Discipline::EveryMessage => commit_if(!media_only),
        }
    }
}

/// 只有违规旗成立（群聊内核 `Conditions` 的同名判断是私有的，这里照它写一份）。
fn only_flagged(conditions: &Conditions) -> bool {
    !conditions.hits.is_empty()
        && conditions
            .hits
            .iter()
            .all(|hit| hit.kind == Kind::Moderation)
}

#[cfg(test)]
mod tests;
