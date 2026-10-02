//! 一次性入口把增量拼成整段回答（施工 8-20，`docs/blueprint/models.md`「怎么走」第十二条第 4 条）：照内核的累积器拼，正文块
//! 的字照先后接起来，思考不要。增量对不上的（驱动的错）交 `model_failed`，分类 `bad_stream`。

use miyu_http::Progress;
use miyu_kernel::accumulate::{Accumulator, DeltaError};
use miyu_kernel::block::Block;
use miyu_kernel::event::{CallError, ErrorClass, Usage};
use miyu_kernel::id::Seq;
use miyu_models::provider::Target;

use super::{Answer, Unanswered};

/// 收到的增量。
#[derive(Default)]
pub(super) struct Reply {
    accumulator: Accumulator,
    /// 第一处对不上的；有了以后不再收。
    broken: Option<DeltaError>,
    /// 发出去了（施工 8-15）：发出去了的才记账。
    sent: bool,
}

impl Reply {
    /// 收下一样：增量照累积器拼，「发出去了」只记下发出去了。
    pub(super) fn take(&mut self, progress: Progress) {
        let Progress::Delta(delta) = progress else {
            self.sent = true;
            return;
        };
        if self.broken.is_none()
            && let Err(error) = self.accumulator.apply(delta)
        {
            self.broken = Some(error);
        }
    }

    /// 发出去了没有（施工 8-15）。
    pub(super) fn sent(&self) -> bool {
        self.sent
    }

    /// 正常说完了：发给了 `target`，用量 `usage`。
    pub(super) fn answer(
        self,
        target: &Target,
        usage: Option<Usage>,
    ) -> Result<Answer, Unanswered> {
        if let Some(error) = self.broken {
            return Err(Unanswered::Failed(CallError {
                class: ErrorClass::BadStream,
                message: error.to_string(),
                status: None,
            }));
        }
        let text = self
            .accumulator
            .finish(Seq::FIRST)
            .into_iter()
            .filter_map(|block| match block {
                Block::Text(text) => Some(text.text),
                _ => None,
            })
            .collect();
        Ok(Answer {
            text,
            provider: target.provider.id.clone(),
            model: target.model.clone(),
            usage,
        })
    }
}
