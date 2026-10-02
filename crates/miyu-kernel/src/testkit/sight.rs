//! 替身替它看图（施工 8-17，`docs/blueprint/kernel/session.md`「替它看图」）：内核交出来的转述请求记下来，照剧本回。转述和
//! 主请求分开记、分开排剧本：不占主请求的剧本，也不算在 [`Stage::requests`] 里。没排剧本就来转述是测试写错了，当场 panic。

use std::collections::VecDeque;

use super::Stage;
use crate::id::{ContentHash, ModelName, ProviderId};
use crate::origin::Model;
use crate::request::Request;
use crate::session::Input;

/// 替身的转述剧本和交出来的请求。
#[derive(Debug, Default)]
pub(super) struct Sight {
    /// 接下来几次怎么回：`Some` 是成了的转述，`None` 是没成。
    answers: VecDeque<Option<String>>,
    /// 交出来的转述请求，照先后：哪一张图、请求。
    asked: Vec<(ContentHash, Request)>,
    /// 先扣着、不马上送回（[`Stage::hold_vision`]）。
    hold: bool,
    /// 扣着的，照交出来的先后：哪一张图、照剧本要回的。
    held: Vec<(ContentHash, Option<String>)>,
}

/// 替身里替它看图的模型：`bigmodel/glm-5.3-flash`。
///
/// # Panics
///
/// 实际不会 panic：名字合写法。
pub fn vision_model() -> Model {
    Model {
        endpoint: ProviderId::parse("bigmodel").expect("端点编号合写法"),
        model: ModelName::parse("glm-5.3-flash").expect("模型名合写法"),
    }
}

impl Stage {
    /// 替它看图接下来几次，照交出来的先后这样回：`Some` 是转述成了（替它看的是 [`vision_model`]），`None` 是没成。
    pub fn vision<'a>(&mut self, answers: impl IntoIterator<Item = Option<&'a str>>) {
        self.sight
            .answers
            .extend(answers.into_iter().map(|answer| answer.map(str::to_string)));
    }

    /// 交出来的转述请求，照先后：哪一张图，和请求本身。
    pub fn describes(&self) -> &[(ContentHash, Request)] {
        &self.sight.asked
    }

    /// 从现在起转述先扣着，不马上送回：好在看图的时候插手（打断、撤销、重启）。
    pub fn hold_vision(&mut self) {
        self.sight.hold = true;
    }

    /// 不再扣着，扣着的照交出来的先后送回。
    pub fn release_vision(&mut self) {
        self.sight.hold = false;
        let held = std::mem::take(&mut self.sight.held);
        let inputs: Vec<Input> = held
            .into_iter()
            .map(|(blob, answer)| self.described(blob, answer))
            .collect();
        self.drain(inputs.into());
    }

    /// 转述一张图：记下，照剧本回；扣着的先放着。
    pub(super) fn describe(&mut self, blob: ContentHash, request: Request) -> Vec<Input> {
        self.sight.asked.push((blob.clone(), request));
        let answer = self
            .sight
            .answers
            .pop_front()
            .unwrap_or_else(|| panic!("剧本里没排第 {} 次转述怎么回", self.sight.asked.len()));
        if self.sight.hold {
            self.sight.held.push((blob, answer));
            return Vec::new();
        }
        vec![self.described(blob, answer)]
    }

    /// 转述回来了：成了的是 [`vision_model`] 看的。
    fn described(&mut self, blob: ContentHash, answer: Option<String>) -> Input {
        Input::Described {
            at: self.tick(),
            blob,
            seen: answer.map(|text| (vision_model(), text)),
        }
    }
}
