//! 一条后台命令到这时为止的输出（`protocol.md` 的 `job.output`，施工 7-4 补）：头照 1 秒读一次，推增量随 M8。

use serde_json::Value;

/// `job.output` 的回应。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JobOutput {
    /// 交的字：最后 `tail` 行，照原样接起来，没清过转义序列。
    pub text: String,
    /// 一共几行。
    pub lines: u64,
    /// 还在跑（回报还没落盘）。
    pub running: bool,
    /// 前面还有没交的。
    pub truncated: bool,
}

impl JobOutput {
    /// 照回应的 `result` 读。
    pub fn read(result: &Value) -> Self {
        Self {
            text: result["output"].as_str().unwrap_or_default().to_string(),
            lines: result["lines"].as_u64().unwrap_or(0),
            running: result["running"].as_bool().unwrap_or(false),
            truncated: result["truncated"].as_bool().unwrap_or(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::JobOutput;

    #[test]
    fn the_answer_of_job_output_is_read() {
        let result =
            json!({"lines": 2, "output": "building\nhalf\n", "running": true, "truncated": false});
        assert_eq!(
            JobOutput::read(&result),
            JobOutput {
                text: "building\nhalf\n".into(),
                lines: 2,
                running: true,
                truncated: false,
            }
        );
    }
}
