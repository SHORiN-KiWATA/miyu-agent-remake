// @ts-check
//! 时间线的段和步（蓝图 `web.md`「时间线的数」，照 TUI 演示的 `transcript/steps.rs`、`blocks.rs`）：她开口之前的一串
//! 步骤，思考、调工具。
//!
//! - 思考、调工具记进在进行的那一段；她开口（`text` 一块）、这一轮结束，这一段收起（`tui.md`「时间线」第 1 条）；
//! - 工具的结果照调用编号找回那一步；这一轮结束时还没结果的记成 `cancelled`、停表（第 13 条）；
//! - 用时照记下的时刻（`core/store.js` 看着流出来时记的）；读回来的历史，工具从回复落盘算到结果，思考照 `model.called`
//!   的 `blocks`（施工 2-3 补；旧日志没有这一格的不知道）。
//!
//! 段是正文的一条（`type: 'steps'`），编号照回合和这一轮的第几段；一步的编号照请求和这次请求里第几块这一种
//! （`k请求-种类-第几块`）：在收的块落了盘，段和步都照旧是它，点开的、淡入过的不重来。

import { res } from '../util/res.js';

/**
 * @typedef {{key: string, kind: 'thought', text: string, state: 'thinking'|'done', start: number|null, end: number|null}} Thought
 * @typedef {{key: string, kind: 'tool', name: string, args: string, parsed: any, state: 'preparing'|'running'|'done',
 *   status: string|null, output: string, said: {key: string, fields: Record<string, string>}|null, callId: string|null,
 *   start: number|null, end: number|null, duration: number|null, job?: string|null, toTitle?: string|null}} Tool
 *   `duration` 是 `tool.result` 的 `duration_ms`（执行命令写在名字后面）；`job` 是结果里派出去的任务的编号（派子代理那一行写它）；
 *   `toTitle` 是留言发给的那个子代理的标题（派它的那一步的 `description`，点开写在「发给」里；找不到的是 `null`）
 * @typedef {Thought|Tool} Step
 * @typedef {{type: 'steps', key: string, turn: number, finished: boolean, steps: Step[]}} Segment
 * @typedef {{kind: string, name?: string, text: string, done: boolean, start: number|null, end: number|null,
 *   mark?: {start: number, end: number|null}}} Block 在收的一块（`core/store.js`）；`mark` 是记下的时刻那一格
 */

export class Timeline {
  /**
   * @param {any[]} items 正文的条目：新的一段接在后面
   * @param {Map<string, {start: number, end: number|null}>} marks 看着流出来时记下的时刻：`请求:种类:第几块` → 开始、收全
   */
  constructor(items, marks) {
    this.items = items;
    this.marks = marks;
    /** @type {Map<string, Tool>} 调用编号 → 那一步 */
    this.calls = new Map();
    /** @type {Map<number, number>} 每一轮开过几段 */
    this.count = new Map();
    /** @type {Map<string, Thought>} 落了盘的思考：`请求:回复里的第几块` → 那一步，`model.called` 的 `blocks` 照它对上 */
    this.placed = new Map();
    /** @type {Map<string, string|null>} 这个会话派出去的任务：编号 → 标题（派它的那一步的 `description`），留言照它认发给谁 */
    this.jobs = new Map();
  }

  /** 在进行的那一段：最后一条是这一轮没收起的一段就是它，不是就另起一段。 */
  open(turn) {
    const last = this.items.at(-1);
    if (last?.type === 'steps' && last.turn === turn && !last.finished) return last;
    const n = (this.count.get(turn) ?? 0) + 1;
    this.count.set(turn, n);
    /** @type {Segment} */
    const segment = { type: 'steps', key: `t${turn}-${n}`, turn, finished: false, steps: [] };
    this.items.push(segment);
    return segment;
  }

  /** 她开口了：收起在进行的那一段（说话就收起）。`at` 是开口的那一刻，在想的停在这里。 */
  speak(at) {
    const last = this.items.at(-1);
    if (last?.type === 'steps' && !last.finished) finish(last, at);
  }

  /**
   * 落了盘的回复里的一块思考、调工具。`n` 是这次请求里第几块这一种（对上记下的时刻，也是这一步的编号），`k` 是回复里
   * 的第几块（对上 `model.called` 的 `blocks`）。
   * @param {any} e `message.assistant`
   */
  persisted(e, block, n, k) {
    const mark = this.marks.get(`${e.body.seen}:${block.type}:${n}`);
    const key = `k${e.body.seen}-${block.type}-${n}`;
    if (block.type === 'reasoning') {
      /** @type {Thought} */
      const step = { key, kind: 'thought', text: block.text, state: 'done', start: mark?.start ?? null, end: mark?.end ?? null };
      this.placed.set(`${e.body.seen}:${k}`, step);
      this.open(e.turn).steps.push(step);
      return;
    }
    // 读回来的历史不知道参数什么时候开始写：从回复落盘算起
    const step = tool(key, block.name, block.args, 'running', mark?.start ?? Date.parse(e.at));
    step.callId = block.call_id;
    this.calls.set(block.call_id, step);
    this.open(e.turn).steps.push(step);
  }

  /**
   * 在收的一块（`core/store.js` 攒的）：思考接字；调工具的参数收全以前是「准备」（`tui.md`「时间线」第 9 条）。
   * `n` 是这次请求里第几块这一种。
   * @param {Block} block
   */
  streaming(turn, seen, block, n) {
    const key = `k${seen}-${block.kind}-${n}`;
    if (block.kind === 'reasoning') {
      this.open(turn).steps.push({ key, kind: 'thought', text: block.text, state: block.done ? 'done' : 'thinking', start: block.start, end: block.end });
      return;
    }
    this.open(turn).steps.push(tool(key, block.name ?? '', block.text, block.done ? 'running' : 'preparing', block.start));
  }

  /**
   * 一次请求的记录（`model.called`）：读回来的历史里思考想了多久，照 `blocks` 算（施工 2-3 补，`kernel/events-bodies.md`）。
   * 一块一项 `{start_ms, end_ms}`，照回复的块的先后，从请求发出去算起；发出去的时刻是 `at` 减 `duration_ms`。
   * 看着流出来时记下的优先。
   */
  called(e) {
    const b = e.body;
    if (!Array.isArray(b.blocks) || b.duration_ms == null) return;
    const sent = Date.parse(e.at) - b.duration_ms;
    b.blocks.forEach((t, k) => {
      const step = this.placed.get(`${b.seen}:${k}`);
      if (!step || step.start != null) return;
      step.start = sent + t.start_ms;
      step.end = sent + t.end_ms;
    });
  }

  /** 工具的结果：照调用编号找回那一步，记下状态、输出、结果那一句，停表。 */
  result(e) {
    const b = e.body;
    const step = this.calls.get(b.call_id);
    if (!step) return;
    step.state = 'done';
    step.status = b.status;
    step.output = (b.blocks ?? []).filter((x) => x.type === 'text').map((x) => x.text).join('\n');
    // 结果里的图（`read` 读图）：点开那一步时画，照核心收下的那一份 blob 取（蓝图「图片」第 2 条）
    step.images = (b.blocks ?? []).filter((x) => x.type === 'image' && x.blob).map((x) => ({ blob: x.blob, media_type: x.media_type, width: x.width, height: x.height }));
    step.said = b.human ?? null;
    // 派出去的任务的编号（派子代理那一行写它），记下它的标题；留言对上发给的那一个（`to` 是任务编号，前面几轮派的也认得）
    step.job = (b.effects ?? []).find((fx) => fx.kind === 'job.started')?.job ?? null;
    if (step.job) this.jobs.set(step.job, typeof step.parsed?.description === 'string' ? step.parsed.description : null);
    const to = step.parsed?.to;
    if (res.timeline.kinds[step.name] === 'message' && typeof to === 'string') step.toTitle = this.jobs.get(to) ?? null;
    step.duration = b.duration_ms ?? null;
    step.end = Date.parse(e.at);
  }

  /** 一轮结束：这一轮的段都收起；还没结果的记成打断，停表（`tui.md`「时间线」第 13 条）。 */
  end(turn, at) {
    for (const it of this.items) {
      if (it.type !== 'steps' || it.turn !== turn) continue;
      finish(it, at);
      for (const step of it.steps) {
        if (step.kind !== 'tool' || step.state === 'done') continue;
        step.state = 'done';
        step.status = 'cancelled';
        stop(step, at);
      }
    }
  }
}

/**
 * 在转圈的那一步（`tui.md`「时间线」第 19 条）：她还在写的（思考中、准备……）是最后那一步；都写完了，是最前面那个
 * 没结果的（核心不报哪一个开始跑了，排在前面的就是在跑的那个）。都有结果了是 `null`。
 * @param {Segment} segment
 */
export function active(segment) {
  const last = segment.steps.at(-1);
  if (last && (last.state === 'thinking' || last.state === 'preparing')) return segment.steps.length - 1;
  const i = segment.steps.findIndex((s) => s.state !== 'done');
  return i < 0 ? null : i;
}

/** 调工具的一步：参数收全了读成值，读不懂的是 `null`。 */
function tool(key, name, args, state, start) {
  /** @type {Tool} */
  const step = { key, kind: 'tool', name, args, parsed: null, state, status: null, output: '', said: null, callId: null, start, end: null, duration: null, job: null };
  if (state !== 'preparing') {
    try { step.parsed = JSON.parse(args); } catch { step.parsed = null; }
  }
  return step;
}

/** 收起一段：在想的停表。`at` 不知道（`null`）的只收起。 */
function finish(segment, at) {
  segment.finished = true;
  for (const step of segment.steps) {
    if (step.kind !== 'thought' || step.state !== 'thinking') continue;
    step.state = 'done';
    stop(step, at);
  }
}

/** 停表：知道什么时候开始的、还没停的，停在 `at`。 */
function stop(step, at) {
  if (step.start != null && step.end == null && at != null) step.end = at;
}
