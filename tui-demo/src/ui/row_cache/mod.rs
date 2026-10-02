//! 正文按条缓存排好的行（蓝图 `tui.md`「正文」第 8 条）：每一条排好的行按编号记着，这一条自己和排版的条件都没变
//! 就直接拿记着的用，只重排变了的、在进行的那几条。排好的行拼成一份 [`Rows`]，视口、鼠标、复制直接引用，
//! 不再每帧复制。

use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::mem::discriminant;
use std::rc::Rc;
use std::time::{Duration, Instant};

use super::rows::{self, Ctx, Row, Target};
use crate::theme;
use crate::transcript::{Entry, Kind, Step, StepKind, ToolState};

/// 这一帧的全部行：一条一块，共享记着的那一份。
#[derive(Debug, Clone, Default)]
pub struct Rows {
    chunks: Vec<Rc<[Row]>>,
    /// 每一块结束在第几行（不含）。
    ends: Vec<usize>,
    /// 每一块是第几条的（条目间的空行、等她第一个字的那几行不是哪一条的）。
    owners: Vec<Option<usize>>,
}

impl Rows {
    /// 一共几行。
    pub fn len(&self) -> usize {
        self.ends.last().copied().unwrap_or(0)
    }

    /// 一行都没有。
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 第 `i` 行。
    pub fn get(&self, i: usize) -> Option<&Row> {
        let k = self.ends.partition_point(|&end| end <= i);
        let start = if k == 0 { 0 } else { self.ends[k - 1] };
        self.chunks.get(k)?.get(i - start)
    }

    /// 从头到尾每一行。
    pub fn iter(&self) -> impl Iterator<Item = &Row> {
        self.chunks.iter().flat_map(|chunk| chunk.iter())
    }

    /// 从第 `first` 行起的 `n` 行，带着行号：只画视口里的，不从头数过去。
    pub fn window(&self, first: usize, n: usize) -> impl Iterator<Item = (usize, &Row)> {
        (first..first.saturating_add(n).min(self.len())).filter_map(|i| Some((i, self.get(i)?)))
    }

    /// 最后一块从第几行起（正文最后露出来的那一条）。
    pub fn last_start(&self) -> usize {
        self.ends.len().checked_sub(2).map_or(0, |k| self.ends[k])
    }

    pub(super) fn push(&mut self, chunk: Rc<[Row]>) {
        self.push_owned(chunk, None);
    }

    fn push_owned(&mut self, chunk: Rc<[Row]>, owner: Option<usize>) {
        if chunk.is_empty() {
            return;
        }
        let end = self.len() + chunk.len();
        self.chunks.push(chunk);
        self.ends.push(end);
        self.owners.push(owner);
    }

    /// 第 `row` 行是第几条的；落在条目间空行上的，照它下面那一条。
    pub fn entry_at(&self, row: usize) -> Option<usize> {
        let k = self.ends.partition_point(|&end| end <= row);
        self.owners.get(k..)?.iter().find_map(|o| *o)
    }

    /// 第 `entry` 条的最后一行后面是第几行（不含）；没有这一条的是 `None`。
    pub fn end_of(&self, entry: usize) -> Option<usize> {
        let k = self.owners.iter().rposition(|o| *o == Some(entry))?;
        Some(self.ends[k])
    }

    /// 第 `entry` 条从第几行起。
    pub fn start_of(&self, entry: usize) -> Option<usize> {
        let k = self.owners.iter().position(|o| *o == Some(entry))?;
        Some(if k == 0 { 0 } else { self.ends[k - 1] })
    }
}

/// 这一帧怎么排（蓝图「正文」第 8 条）：要整份重排时一帧最多花多久，先排哪一条附近的。
#[derive(Debug, Clone, Copy, Default)]
pub struct Plan {
    /// 这一帧重排最多花多久；`None` 是不限（一口气排完）。每帧至少排一条。
    pub budget: Option<Duration>,
    /// 视口附近是第几条：从它往两边排；`None` 是跟着最新，从最底下往上排。
    pub anchor: Option<usize>,
}

impl Plan {
    /// 不限时，一口气排完。
    #[cfg(test)]
    pub fn all() -> Self {
        Self::default()
    }
}

impl From<Vec<Row>> for Rows {
    fn from(rows: Vec<Row>) -> Self {
        let mut out = Self::default();
        out.push(rows.into());
        out
    }
}

/// 记着的行：条目的编号到（指纹、排好的行、排出来带不带图）。
#[derive(Debug, Default)]
pub struct RowCache {
    entries: HashMap<u64, (u64, Rc<[Row]>, bool)>,
    /// 上一帧重排了几条（测试、量尺看）。
    pub rebuilt: usize,
    /// 上一帧没轮到、先用旧行的有几条：还有的话下一帧接着排（`App::deadline`）。
    pub stale: usize,
    /// 上一帧重排的是哪几条（编号）。
    fresh: HashSet<u64>,
}

impl RowCache {
    /// 编号 `id` 的那一条上一帧重排过。
    #[cfg(test)]
    pub fn fresh(&self, id: u64) -> bool {
        self.fresh.contains(&id)
    }
}

/// 把看得见的正文排成行，一条之间空一行；没变的条目用记着的。和整份重排（测试里的 `fresh_rows`）排出来的一模一样。
/// 要重排的多、`plan` 给了预算的，照预算排视口附近的几条，没轮到的先用旧行，记进 `stale`，下一帧接着排。
pub fn build(entries: &[Entry], ctx: &Ctx, cache: &RefCell<RowCache>, plan: Plan) -> Rows {
    let figures = ctx.figures.borrow().revision();
    let mut cache = cache.borrow_mut();
    cache.rebuilt = 0;
    cache.stale = 0;
    cache.fresh.clear();
    let blank: Rc<[Row]> = Rc::from(vec![ctx.row(ctx.blank_slot(), Vec::new())]);
    let shown: Vec<usize> = (0..entries.len())
        .filter(|&i| rows::shown(&entries[i]))
        .collect();
    // 先看哪几条能直接用记着的，别的记下来要排。
    let mut chunks: Vec<Option<Rc<[Row]>>> = vec![None; shown.len()];
    let mut todo = Vec::new();
    for (k, &i) in shown.iter().enumerate() {
        let entry = &entries[i];
        // 图做好了几张只算上次排出来带图的条目（没记着的照样算上，反正要排）。
        let with_figures = cache.entries.get(&entry.id).is_none_or(|kept| kept.2);
        let key = fingerprint(i, entry, ctx, with_figures.then_some(figures));
        match (key, cache.entries.get(&entry.id)) {
            (Some(key), Some((old, rows, _))) if *old == key => chunks[k] = Some(rows.clone()),
            _ => todo.push(k),
        }
    }
    // 从视口附近往两边排：跟着最新的从最底下往上。
    let near = plan
        .anchor
        .and_then(|a| shown.iter().position(|&i| i >= a))
        .unwrap_or(shown.len().saturating_sub(1));
    todo.sort_by_key(|&k| k.abs_diff(near));
    let start = Instant::now();
    for k in todo {
        let i = shown[k];
        let entry = &entries[i];
        let over = cache.rebuilt > 0 && plan.budget.is_some_and(|b| start.elapsed() >= b);
        // 预算用完了：记着旧行、不是在进行的，这一帧先用旧行。
        let stale = over
            .then(|| cache.entries.get(&entry.id).map(|kept| kept.1.clone()))
            .flatten()
            .filter(|_| fingerprint(i, entry, ctx, None).is_some());
        if let Some(rows) = stale {
            cache.stale += 1;
            chunks[k] = Some(rows);
            continue;
        }
        cache.rebuilt += 1;
        cache.fresh.insert(entry.id);
        let rows: Rc<[Row]> = rows::entry_rows(i, entry, ctx).into();
        let has_figures = rows.iter().any(|r| r.figure.is_some() || r.figure_pending);
        if let Some(key) = fingerprint(i, entry, ctx, has_figures.then_some(figures)) {
            cache
                .entries
                .insert(entry.id, (key, rows.clone(), has_figures));
        }
        chunks[k] = Some(rows);
    }
    let mut out = Rows::default();
    for (chunk, &i) in chunks.into_iter().zip(&shown) {
        if !out.is_empty() {
            out.push(blank.clone());
        }
        if let Some(chunk) = chunk {
            out.push_owned(chunk, Some(i));
        }
    }
    // 不在了的（撤销后又被删掉、排队退回的）不留着。
    let seen: HashSet<u64> = shown.iter().map(|&i| entries[i].id).collect();
    cache.entries.retain(|id, _| seen.contains(id));
    out
}

/// 这一条和排版条件的指纹：变了就重排。在进行的那一段（还有步骤在转圈、在走表）、正在压缩的那一行（行首在转圈）
/// 是 `None`，每帧重排。
fn fingerprint(i: usize, entry: &Entry, ctx: &Ctx, figures: Option<u64>) -> Option<u64> {
    if let Some(segment) = &entry.segment
        && (!segment.finished || segment.steps.iter().any(Step::busy))
    {
        return None;
    }
    if entry.progress.is_some() {
        return None;
    }
    let mut h = DefaultHasher::new();
    // 排版的条件：行里带着条目的位置（点中的东西），颜色、图标烤在行里，图占几行看图做好没有（只算带图的条目）。
    // 权限级别只有没记下当时级别的「你说的话」用得上：它的竖线照现在的级别上色（「正文」第 8 条）。
    let level = (matches!(entry.kind, Kind::User) && entry.level.is_none()).then_some(ctx.level);
    (
        i,
        ctx.width,
        ctx.indent.len(),
        level,
        theme::generation(),
        &ctx.config.icons.name,
        // 语言连同是不是自动：自动的中文和手动的中文代码一样，收起那一行却一个英文一个中文。
        (ctx.config.language.code(), ctx.config.auto),
        figures,
    )
        .hash(&mut h);
    ctx.hover.filter(|t| owner(*t) == i).hash(&mut h);
    // 这一条自己：字只看长短和末尾（流式的字只往后接），别的看状态。
    discriminant(&entry.kind).hash(&mut h);
    let text = entry.text.as_bytes();
    (text.len(), &text[text.len().saturating_sub(64)..]).hash(&mut h);
    (entry.hidden, entry.queued, entry.open, entry.level).hash(&mut h);
    (
        entry.undo.is_some(),
        entry.pasted.len(),
        &entry.details,
        &entry.from,
    )
        .hash(&mut h);
    if let Some(job) = &entry.job {
        (discriminant(&job.mark), job.detail.len()).hash(&mut h);
    }
    if let Some(segment) = &entry.segment {
        (segment.finished, segment.open, segment.steps.len()).hash(&mut h);
        for step in &segment.steps {
            (step.took, step.open).hash(&mut h);
            match &step.kind {
                StepKind::Thought { text } => text.len().hash(&mut h),
                StepKind::Tool {
                    name,
                    args,
                    state,
                    output,
                    said,
                    ..
                } => {
                    (name, args.len(), output.len(), said.is_some()).hash(&mut h);
                    discriminant(state).hash(&mut h);
                    if let ToolState::Done(status) = state {
                        discriminant(status).hash(&mut h);
                    }
                }
            }
        }
    }
    Some(h.finish())
}

/// 点中的东西属于第几条。
fn owner(target: Target) -> usize {
    match target {
        Target::Segment(i) | Target::Step(i, _) | Target::Entry(i) | Target::Details(i, _) => i,
    }
}

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
