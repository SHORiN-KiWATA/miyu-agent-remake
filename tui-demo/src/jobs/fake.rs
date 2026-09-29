//! 演示用的假数据源（蓝图 `tui.md`「后台命令、子代理和侧边栏」）：照 `resources/fake.json` 的脚本，
//! 隔一阵往后台任务表、待办里推一步。核心有了后台任务表以后整个换掉。

use std::time::{Duration, Instant};

use serde::Deserialize;

use super::{Board, Job, JobKind, JobState, Todo, TodoState};

/// 一条假的后台命令。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShellScript {
    /// 命令。
    pub command: String,
    /// 一行一行的输出。
    pub lines: Vec<String>,
    /// 隔多久出一行。
    pub every_ms: u64,
    /// 退出码：0 是成功。
    pub exit: i32,
}

/// 一个假的子代理。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentScript {
    /// 名字。
    pub name: String,
    /// 每一步「正在做什么」。
    pub steps: Vec<String>,
    /// 隔多久换一步。
    pub every_ms: u64,
    /// 每一步用掉多少 token。
    pub tokens_per_step: u64,
    /// 走完交回来的报告。
    pub report: String,
}

/// 一份假的待办。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TodoScript {
    /// 各项。
    pub items: Vec<String>,
    /// 隔多久推进一项。
    pub every_ms: u64,
}

/// 整个脚本（`resources/fake.json`）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Script {
    /// 后台命令，`/demo-bg` 轮着起。
    pub shells: Vec<ShellScript>,
    /// 子代理，`/demo-agent` 轮着派。
    pub agents: Vec<AgentScript>,
    /// 待办，`/demo-todo` 推。
    pub todos: TodoScript,
}

/// 正在演的一段。
#[derive(Debug)]
enum Play {
    Shell { id: u64, script: usize, step: usize },
    Agent { id: u64, script: usize, step: usize },
    Todo { step: usize },
}

/// 假数据源：在演的几段，和各自下一步的时刻。
#[derive(Debug, Default)]
pub struct Feed {
    plays: Vec<(Play, Instant)>,
    next_id: u64,
    shells: usize,
    agents: usize,
}

impl Feed {
    /// 起一条后台命令（脚本轮着用）。
    pub fn start_shell(&mut self, script: &Script, board: &mut Board, now: Instant) {
        let Some(index) = (!script.shells.is_empty()).then(|| self.shells % script.shells.len())
        else {
            return;
        };
        self.shells += 1;
        let s = &script.shells[index];
        let id = self.job(board, JobKind::Shell, &s.command, now);
        let next = now + Duration::from_millis(s.every_ms);
        self.plays.push((
            Play::Shell {
                id,
                script: index,
                step: 0,
            },
            next,
        ));
    }

    /// 派一个子代理（脚本轮着用）。
    pub fn start_agent(&mut self, script: &Script, board: &mut Board, now: Instant) {
        let Some(index) = (!script.agents.is_empty()).then(|| self.agents % script.agents.len())
        else {
            return;
        };
        self.agents += 1;
        let a = &script.agents[index];
        let id = self.job(board, JobKind::Agent, &a.name, now);
        if let Some(job) = board.jobs.iter_mut().find(|j| j.id == id) {
            job.doing = a.steps.first().cloned().unwrap_or_default();
            job.tokens = a.tokens_per_step;
        }
        let next = now + Duration::from_millis(a.every_ms);
        self.plays.push((
            Play::Agent {
                id,
                script: index,
                step: 1,
            },
            next,
        ));
    }

    /// 推一份待办：第一项在做，别的没做。
    pub fn start_todo(&mut self, script: &Script, board: &mut Board, now: Instant) {
        board.todos = script
            .todos
            .items
            .iter()
            .enumerate()
            .map(|(i, text)| Todo {
                text: text.clone(),
                state: if i == 0 {
                    TodoState::Active
                } else {
                    TodoState::Pending
                },
            })
            .collect();
        self.plays.retain(|(p, _)| !matches!(p, Play::Todo { .. }));
        let next = now + Duration::from_millis(script.todos.every_ms);
        self.plays.push((Play::Todo { step: 0 }, next));
    }

    /// 走到 `now`：到点的推一步。返回这一次结束了的后台任务的编号。
    pub fn advance(&mut self, script: &Script, board: &mut Board, now: Instant) -> Vec<u64> {
        let mut finished = Vec::new();
        let mut keep = Vec::new();
        for (play, at) in std::mem::take(&mut self.plays) {
            if at > now {
                keep.push((play, at));
                continue;
            }
            if let Some(next) = step(play, at, script, board, &mut finished) {
                keep.push(next);
            }
        }
        self.plays = keep;
        // 一次走不完的（落后了好几步）下一帧接着走。
        finished
    }

    /// 下一步的时刻；没有在演的是 `None`。
    pub fn next_at(&self) -> Option<Instant> {
        self.plays.iter().map(|(_, at)| *at).min()
    }

    fn job(&mut self, board: &mut Board, kind: JobKind, title: &str, now: Instant) -> u64 {
        self.next_id += 1;
        board.jobs.push(Job {
            id: self.next_id,
            kind,
            title: title.to_string(),
            state: JobState::Running,
            started: now,
            ended: None,
            output: Vec::new(),
            doing: String::new(),
            tokens: 0,
            report: String::new(),
        });
        self.next_id
    }
}

/// 推一步；还没演完的返回下一步。人停掉了的不再推。
fn step(
    play: Play,
    at: Instant,
    script: &Script,
    board: &mut Board,
    finished: &mut Vec<u64>,
) -> Option<(Play, Instant)> {
    match play {
        Play::Shell {
            id,
            script: i,
            step,
        } => {
            let s = &script.shells[i];
            let job = board.jobs.iter_mut().find(|j| j.id == id && j.running())?;
            if let Some(line) = s.lines.get(step) {
                job.output.push(line.clone());
            }
            if step + 1 >= s.lines.len() {
                job.state = if s.exit == 0 {
                    JobState::Done
                } else {
                    JobState::Failed(s.exit)
                };
                job.ended = Some(at);
                finished.push(id);
                return None;
            }
            let next = at + Duration::from_millis(s.every_ms);
            Some((
                Play::Shell {
                    id,
                    script: i,
                    step: step + 1,
                },
                next,
            ))
        }
        Play::Agent {
            id,
            script: i,
            step,
        } => {
            let a = &script.agents[i];
            let job = board.jobs.iter_mut().find(|j| j.id == id && j.running())?;
            match a.steps.get(step) {
                Some(doing) => {
                    job.doing = doing.clone();
                    job.tokens += a.tokens_per_step;
                    let next = at + Duration::from_millis(a.every_ms);
                    Some((
                        Play::Agent {
                            id,
                            script: i,
                            step: step + 1,
                        },
                        next,
                    ))
                }
                None => {
                    job.report = a.report.clone();
                    job.state = JobState::Done;
                    job.ended = Some(at);
                    finished.push(id);
                    None
                }
            }
        }
        Play::Todo { step } => {
            let todos = &mut board.todos;
            if let Some(t) = todos.get_mut(step) {
                t.state = TodoState::Done;
            }
            if let Some(t) = todos.get_mut(step + 1) {
                t.state = TodoState::Active;
                let next = at + Duration::from_millis(script.todos.every_ms);
                return Some((Play::Todo { step: step + 1 }, next));
            }
            None
        }
    }
}
