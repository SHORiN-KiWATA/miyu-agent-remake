//! 替身和压缩有关的几样（施工 6-2、6-3 上）：交模型限额，回摘要请求。

use super::Stage;
use super::script::Line;
use crate::session::{Input, Limits};

impl Stage {
    /// 交模型限额：替身的模型，窗口和最大输出照给的，图片照策略里的固定数（施工 6-2 上）。
    pub fn limits(&mut self, window: Option<u64>, max_output: Option<u64>) {
        self.limits_with(Limits {
            model: super::respond::model(),
            window,
            max_output,
            images: None,
        });
    }

    /// 交模型限额，照给的原样：带图片算法的用它（施工 6-3 上）。
    pub fn limits_with(&mut self, limits: Limits) {
        self.limits = Some(limits.clone());
        self.run(Input::Limits(limits));
    }

    /// 从现在起，最后一块是 `instruction` 的请求是摘要请求，一律照 `line` 回，不占剧本：随机的剧本事先不知道哪一次
    /// 会压（施工 6-2 上）。
    pub fn summarize_with(&mut self, instruction: &str, line: Line) {
        self.summaries = Some((instruction.to_string(), line));
    }
}
