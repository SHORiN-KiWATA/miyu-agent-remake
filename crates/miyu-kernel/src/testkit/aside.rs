//! 替身的辅助请求（施工 3-8 四补的回顾，五补的起标题，`docs/blueprint/kernel/session.md`「回顾」「起标题」）：要一句回顾、
//! 回顾和起标题的请求怎么回、停住的放行。辅助请求和主请求分开记、分开排剧本：它们不占主请求的剧本，也不算在
//! [`Stage::requests`] 里。
//!
//! 回顾没排剧本就来请求是测试写错了，当场 panic；起标题是内核自己要的，没排剧本的只记下、不回（一直在路上）：不管标题的
//! 测试不用替它排。

use super::Stage;
use super::respond::{deltas, model};
use super::script::Line;
use crate::event::{Purpose, Usage};
use crate::id::{CommandId, Seq};
use crate::request::Request;
use crate::session::{Command, Input};

impl Stage {
    /// 要一句回顾。返回这个命令的编号。
    pub fn recap(&mut self) -> CommandId {
        self.command(Command::Recap)
    }

    /// 回顾的请求接下来几次，照先后这样回。
    pub fn recap_model(&mut self, lines: impl IntoIterator<Item = Line>) {
        self.recap_lines.extend(lines);
    }

    /// 回顾的请求，照先后：照到第几条，和请求本身。
    pub fn recaps(&self) -> &[(Seq, Request)] {
        &self.recaps
    }

    /// 放行停住的那次回顾：送说完了。
    ///
    /// # Panics
    ///
    /// 没有停住的回顾。
    pub fn release_recap(&mut self) {
        let (upto, line) = self
            .held_recap
            .take()
            .unwrap_or_else(|| panic!("没有停住的回顾"));
        let ended = self.aside_ended(Purpose::Recap, upto, &line);
        self.run(ended);
    }

    /// alice 改标题、置顶，改哪样写哪样（施工 3-8 五补测起标题用）。返回这个命令的编号。
    pub fn set_meta(&mut self, title: Option<&str>, pinned: Option<bool>) -> CommandId {
        self.command(Command::SetMeta {
            title: title.map(str::to_string),
            pinned,
        })
    }

    /// 起标题的请求接下来几次，照先后这样回（施工 3-8 五补）。没排的只记下、不回。
    pub fn title_model(&mut self, lines: impl IntoIterator<Item = Line>) {
        self.title_lines.extend(lines);
    }

    /// 起标题的请求，照先后：照到第几条，和请求本身。
    pub fn titles(&self) -> &[(Seq, Request)] {
        &self.titles
    }

    /// 放行停住的那次起标题：送说完了。
    ///
    /// # Panics
    ///
    /// 没有停住的起标题。
    pub fn release_title(&mut self) {
        let (upto, line) = self
            .held_title
            .take()
            .unwrap_or_else(|| panic!("没有停住的起标题"));
        let ended = self.aside_ended(Purpose::Title, upto, &line);
        self.run(ended);
    }

    /// 辅助请求：记下，照各自的剧本回。
    pub(super) fn aside_call(
        &mut self,
        purpose: Purpose,
        upto: Seq,
        request: Request,
    ) -> Vec<Input> {
        let hash = request.hash();
        let line =
            match purpose {
                Purpose::Recap => {
                    self.recaps.push((upto, request));
                    let line = self.recap_lines.pop_front();
                    Some(line.unwrap_or_else(|| {
                        panic!("剧本里没排第 {} 次回顾说什么", self.recaps.len())
                    }))
                }
                Purpose::Title => {
                    self.titles.push((upto, request));
                    self.title_lines.pop_front()
                }
                Purpose::Other(_) => None,
            };
        let Some(line) = line else {
            return Vec::new();
        };
        let mut inputs = vec![Input::AsideSent {
            at: self.tick(),
            purpose: purpose.clone(),
            upto,
            model: model(),
            request: hash,
        }];
        if line.error.is_none() || line.says_something() {
            for delta in deltas(&line) {
                inputs.push(Input::AsideDelta {
                    at: self.tick(),
                    purpose: purpose.clone(),
                    upto,
                    delta,
                });
            }
        }
        if line.hold {
            match purpose {
                Purpose::Title => self.held_title = Some((upto, line)),
                _ => self.held_recap = Some((upto, line)),
            }
        } else {
            inputs.push(self.aside_ended(purpose, upto, &line));
        }
        inputs
    }

    /// 辅助请求 `upto` 说完了：出错的带上分类和原话，说完了的带上用量。
    fn aside_ended(&mut self, purpose: Purpose, upto: Seq, line: &Line) -> Input {
        Input::AsideEnded {
            at: self.tick(),
            purpose,
            upto,
            usage: line.error.is_none().then(|| {
                line.usage.unwrap_or(Usage {
                    uncached: 100,
                    cache_read: 0,
                    cache_write: 0,
                    output: 10,
                })
            }),
            error: line.error.clone(),
        }
    }
}
