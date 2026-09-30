//! 后台任务结束的通知（蓝图 `tui.md`「后台命令、子代理和侧边栏」第 5 条）：正文末尾一行，点开看全文；后台命令的输出读回来
//! 以后照任务编号补进去。

use super::{JobMark, JobNote, Kind, Transcript};

impl Transcript {
    /// 正文末尾一条后台任务结束的通知（蓝图「后台命令、子代理和侧边栏」第 5 条）。
    pub fn job(&mut self, mark: JobMark, text: String, detail: String) {
        self.job_from(mark, text, detail, None);
    }

    /// 同上，记着是哪条任务的（任务编号）：后台命令的输出读回来了照它补进点开看的全文（[`Transcript::fill_job`]）。
    pub fn job_from(
        &mut self,
        mark: JobMark,
        text: String,
        detail: String,
        source: Option<String>,
    ) {
        self.note(Kind::Job, text);
        if let Some(entry) = self.entries.last_mut() {
            entry.job = Some(JobNote {
                mark,
                detail,
                source,
            });
        }
    }

    /// 任务 `source` 结束的那一行点开看的全文换成 `detail`（后台命令的输出读回来了）。
    pub fn fill_job(&mut self, source: &str, detail: String) {
        let note = self
            .entries
            .iter_mut()
            .rev()
            .filter_map(|e| e.job.as_mut())
            .find(|n| n.source.as_deref() == Some(source));
        if let Some(note) = note {
            note.detail = detail;
        }
    }
}
