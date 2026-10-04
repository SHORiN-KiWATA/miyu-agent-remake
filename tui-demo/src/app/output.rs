//! 后台命令的输出（蓝图 `tui.md`「后台命令、子代理和侧边栏」第 3、5 条，`protocol.md` 的 `job.output`）：面板里点开的
//! 那条在跑的，隔一会儿读一次最后几行；一条命令结束了读它全部的（最多 `note_lines` 行），正文里它那一行点开看。

use std::time::{Duration, Instant};

use super::{App, Panel};
use crate::core::{Command, JobOutput};
use crate::jobs::clean::note_detail;

impl App {
    /// 面板里点开的那一条：在跑的隔 `poll_ms` 读一次最后几行；结束了还没读过的读一次。上一次还没回来的不再发。
    pub(super) fn poll_output(&mut self) {
        let Some(Panel::Background { open: Some(id), .. }) = self.panel else {
            return;
        };
        let Some(session) = self.transcript.session.clone() else {
            return;
        };
        let look = &self.config.layout.output;
        let (rows, lines) = (look.panel_rows, look.note_lines);
        let every = Duration::from_millis(look.poll_ms);
        let now = Instant::now();
        let Some(job) = self.board.jobs.iter_mut().find(|j| j.id == id) else {
            return;
        };
        let due = if job.running() {
            job.asked.is_none_or(|at| now >= at + every)
        } else {
            job.output.is_none()
        };
        if job.waiting || !due {
            return;
        }
        let tail = if job.running() { rows } else { lines };
        job.waiting = true;
        job.asked = Some(now);
        let job = job.job.clone();
        self.core.send(Command::Output { session, job, tail });
    }

    /// 会话 `session` 里的命令 `job` 结束了：读它全部的输出，正文里它那一行点开看（第 5 条）。
    pub(super) fn fetch_final(&mut self, session: &str, job: &str) {
        let tail = self.config.layout.output.note_lines;
        if let Some((_, board)) = self.slot(session)
            && let Some(entry) = board.by_job_mut(job)
        {
            entry.waiting = true;
            entry.asked = Some(Instant::now());
            let (session, job) = (session.to_string(), job.to_string());
            self.core.send(Command::Output { session, job, tail });
        }
    }

    /// 核心交回了一条命令的输出：记进那个会话的任务表；结束了的补进正文里它那一行（洗过、去掉末尾的空行；前面还有
    /// 没交的，第一行写 `…`；一个字都没有的写「没有输出」）。
    pub(super) fn took_output(&mut self, session: &str, job: &str, output: Option<JobOutput>) {
        let tab = self.config.layout.output.tab;
        let nothing = self.config.text.jobs.no_output.clone();
        let Some((transcript, board)) = self.slot(session) else {
            return;
        };
        let Some(entry) = board.by_job_mut(job) else {
            return;
        };
        entry.waiting = false;
        let Some(output) = output else {
            return;
        };
        // 正文里那一行点开：命令、空行、输出（照时间线点开一条命令，2026-09-30 项目主人：原来只看得到输出）。
        if !output.running {
            let detail = note_detail(&output, tab, &nothing);
            transcript.fill_job(job, format!("{}\n\n{detail}", entry.title.trim()));
        }
        entry.output = Some(output);
    }
}
