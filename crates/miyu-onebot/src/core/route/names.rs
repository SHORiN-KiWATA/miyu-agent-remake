//! 群消息的正文（`onebot.md` 第一条「群消息」第 5 条，施工 O-22）：段照先后接起来，@ 写成 `@名字`。名字照群成员的缓存
//! （`onebot::Members`）；缓存里没有的经收进这条消息的那个机器人号现在的连接问 `get_group_member_info`，问到了记下。没连着、
//! 过了时、失败、回的名字是空的：写 `@<号>`，这一条里后面缓存里没有的不再问，也写号（「施工时定的」第 59 条）：跟核心的
//! 那一头一条条照先后办，NapCat 一卡，一条 @ 十个人就要等十个时限。

use std::time::Instant;

use super::Route;
use super::fields::{NAME, clean};
use crate::TARGET;
use crate::onebot::{MEMBER_INFO, Piece, Posted, display_name, member_info};

impl Route {
    /// 群号是 `group` 的群里那条 `posted` 交给核心的正文：字照原样接，@ 写成 `@名字`，名字拿不到的写 `@<号>`。
    pub(super) async fn named(&mut self, group: i64, posted: &Posted) -> String {
        let mut text = String::new();
        let mut asking = true;
        for piece in &posted.segments.pieces {
            match piece {
                Piece::Text(words) => text.push_str(words),
                Piece::At(user) => {
                    text.push('@');
                    match self.member(group, *user, posted.bot, &mut asking).await {
                        Some(name) => text.push_str(&name),
                        None => text.push_str(&user.to_string()),
                    }
                }
            }
        }
        text
    }

    /// 群 `group` 里号是 `user` 的人叫什么，洗过（同场所的格的 `name`）：先看缓存；没有的、`asking` 还是真的，经机器人号
    /// `bot` 的连接问一次，问不到的把 `asking` 放成假。
    async fn member(
        &mut self,
        group: i64,
        user: i64,
        bot: i64,
        asking: &mut bool,
    ) -> Option<String> {
        if let Some(name) = self.members.name(group, user, Instant::now()) {
            return clean(name, NAME);
        }
        if !*asking {
            return None;
        }
        let asked = match self.bots.get(bot) {
            Some(link) => link
                .calls
                .call(&link.out, MEMBER_INFO, member_info(group, user))
                .await
                .map_err(|error| format!("{error:?}")),
            None => Err("bot not connected".to_string()),
        };
        let Some(name) = asked
            .as_ref()
            .ok()
            .and_then(|reply| display_name(&reply["data"]))
        else {
            *asking = false;
            let error = asked.err().unwrap_or_default();
            tracing::debug!(target: TARGET, group, user, error, "member name unknown");
            return None;
        };
        let cleaned = clean(&name, NAME);
        self.members.remember(group, user, name, Instant::now());
        cleaned
    }
}
