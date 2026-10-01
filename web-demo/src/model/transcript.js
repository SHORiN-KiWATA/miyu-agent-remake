// @ts-check
//! 事件 → 正文的条目。纯函数：同样的事件出同样的条目。
//!
//! 设计上这是核心的视图投影（`01-架构.md` D1、`04-核心协议.md` P3），还没做（M9）；和 TUI 演示一样先在头里算，
//! 规矩照蓝图 `tui.md`「正文」第 4、5、7 条和「时间线」。条目有六种：你的话（别人说的也是这一种，带着是谁，`speaker`）、她的回答
//! （原文，Markdown 由 `ui/chat.js` 画）、时间线的一段（`timeline.js`）、收尾那一行、一轮开始她还一块都没来时的三个球（`waiting`，
//! 蓝图 `web.md`「她出第一个字之前」），和不挂在她头下的一行（`note`：后台命令、子代理的回报，压缩、清空，`notes.js`）。
//!
//! - 你的话等 `turn.started` 来了归到那一轮（还没归的都归进去）；开这一轮的那一句（`trigger`）记下 `opens`，编辑只在它上面；
//! - 回合中途到的话（写着 `turn`）先排着，不进正文（蓝图 `web.md`「排队的消息」）：这一轮后来的请求听到了（`seen` 够着它的
//!   序号），前面那段收起、它挪到正文末尾；没听到、接着开了下一轮，它是下一轮的开头；没在回答了还排着的也进正文；
//! - 撤销了的回合，它的条目都不画；恢复了又画回来；撤回的话（`message.withdrawn`）不画；
//! - 收尾那一行照这一轮开始时的权限级别写图标；人要的压缩、清空单开的那一轮（没有 `trigger`）不另起收尾行，手动压缩的用时和
//!   用量接在压缩那一行后面（蓝图 `web.md`「压缩、清空」）。

import { res, t } from '../util/res.js';
import { short, hitRate, seconds, hhmm } from './format.js';
import { text, attachments } from './session.js';
import { Timeline } from './timeline.js';
import { noteJobs, speakerOf, reportNote, peerNote, changeNote, modelNote, compactedNote, failureText, recapNote, compactFailedNote } from './notes.js';
import { tasksOf, running as runningJobs } from '../lib/jobs.js';

/** 权限：只读开着是只读，关着照常用的那一级（`kernel/events-bodies.md`「权限」）。 */
export const levelOf = (p) => (p?.read_only ? 'read_only' : p?.level ?? 'workspace');

/**
 * @param {any[]} events 持久事件，照序号
 * @param {{turn: number, seen: number, blocks: import('./timeline.js').Block[]}|null} [live]
 *   在收的那一次回复：瞬时的 `model.delta` 攒起来的（`core/store.js`）
 * @param {Map<string, {start: number, end: number|null}>} [marks] 看着流出来时记下的每一块的时刻（`core/store.js`）
 * @param {Map<number, {before: number, after: number}>} [stats] 看着压好的那几次压缩的前后用量，照落了盘的那一条的序号（`core/store.js`）
 */
export function project(events, live = null, marks = new Map(), stats = new Map()) {
  /** @type {any[]} */
  const items = [];
  const turns = new Map();
  const reverted = new Set();
  const timeline = new Timeline(items, marks);
  /** 出过字的回合：来过一条回复 */
  const spoke = new Set();
  /** 排着的话：回合中途来的、还没被请求听到的 */
  const queue = [];
  /** 派出去的任务：回报、子代理说的话照它找标题 */
  const jobs = new Map();
  /** 调用编号 → 参数：后台命令的回报点开时写命令 */
  const args = new Map();
  /** 这个会话是子会话的：派它的那个会话（它发来的话写「派它的会话」） */
  let parent = null;
  /** 请求 `seen` 听到了排着的：前面那段收起，听到的照先后挪到正文末尾 */
  const hear = (seen) => {
    const heard = queue.filter((q) => q.seq <= seen);
    if (!heard.length) return;
    timeline.speak(null);
    for (const q of heard) queue.splice(queue.indexOf(q), 1);
    items.push(...heard);
  };
  let level = 'workspace';
  /** 开过的轮，照先后：回顾记它讲到哪一轮 */
  const started = [];
  for (const e of events) {
    const b = e.body;
    switch (e.kind) {
      case 'session.created':
      case 'session.policy_changed':
        if (e.kind === 'session.created') parent = b.parent ?? null;
        if (b.permission) level = levelOf(b.permission);
        // 钉着的模型没了、核心退回默认的（核心施工 8-10）：一行「X 没了，换回 Y」。人换的不画：换模型、换思考强度都不出提示，
        // 只改框下面那一截（2026-10-01 项目主人定）
        if (e.kind === 'session.policy_changed' && b.model && b.replaced) {
          timeline.speak(Date.parse(e.at));
          items.push(modelNote(e));
        }
        break;
      case 'message.user': {
        const item = { type: 'user', key: `u${e.seq}`, seq: e.seq, turn: e.turn ?? null, text: text(e), attachments: attachments(e), speaker: speakerOf(e.by, jobs, parent), opens: false };
        // 她正在回答时来的先排着，被这一轮后来的请求听到了（`seen` 够着它）才进正文，这一轮都没听到的是下一轮的开头。不带回合编号
        // 的（子代理、别的 harness 发来的）一样（2026-09-30 项目主人指出：原来照日志里的位置直接排，这次请求没听到它也画在她的回答
        // 上面，像她回了它、又莫名开了一轮）
        const running = e.turn != null ? turns.has(e.turn) && !turns.get(e.turn).ended : [...turns.values()].some((x) => !x.ended);
        if (running) queue.push(item);
        else items.push(item);
        break;
      }
      case 'message.withdrawn': {
        const gone = new Set(b.messages);
        for (let i = queue.length - 1; i >= 0; i--) if (gone.has(queue[i].seq)) queue.splice(i, 1);
        for (let i = items.length - 1; i >= 0; i--) if (items[i].type === 'user' && gone.has(items[i].seq)) items.splice(i, 1);
        break;
      }
      case 'turn.started': {
        started.push(e.turn);
        // 排着接着开的：到 `trigger` 为止排着的进正文，是这一轮的开头
        const carried = b.trigger != null ? queue.filter((q) => q.seq <= b.trigger) : [];
        for (const q of carried) queue.splice(queue.indexOf(q), 1);
        items.push(...carried);
        // 人要的压缩、清空单开的那一轮没有 `trigger`（`kernel/events-bodies.md`）
        turns.set(e.turn, { start: e.at, level, calls: [], by: null, manual: b.trigger == null, note: null });
        const mine = items.filter((it) => it.type === 'user' && (it.turn == null || carried.includes(it)));
        for (const it of mine) it.turn = e.turn;
        // 开这一轮的那一句：照 `trigger`；以前的日志没有的，照归进来的最后一句
        const opener = b.trigger != null ? mine.find((it) => it.seq === b.trigger) : mine.at(-1);
        if (opener) opener.opens = true;
        break;
      }
      case 'message.assistant': {
        hear(b.seen);
        if (turns.has(e.turn)) turns.get(e.turn).by = e.by;
        spoke.add(e.turn);
        // 这次请求里第几块这一种：对上看着流出来时记下的时刻
        const nth = new Map();
        b.blocks.forEach((block, k) => {
          if (block.type === 'tool_call') args.set(block.call_id, block.args);
          const n = nth.get(block.type) ?? 0;
          nth.set(block.type, n + 1);
          if (block.type === 'text') {
            timeline.speak(marks.get(`${b.seen}:text:${n}`)?.start ?? Date.parse(e.at));
            items.push({ type: 'reply', key: `r${e.seq}-${k}`, turn: e.turn, text: block.text, streaming: false });
          } else if (block.type === 'reasoning' || block.type === 'tool_call') {
            timeline.persisted(e, block, n, k);
          }
        });
        break;
      }
      case 'tool.result':
        noteJobs(e, jobs, args);
        timeline.result(e);
        break;
      case 'job.reported':
      case 'child.reported':
        // 她正在回答时来的：在进行的那段时间线收起，她接着的步另起一段排在这一行下面
        timeline.speak(Date.parse(e.at));
        items.push(reportNote(e, jobs));
        break;
      case 'model.changed':
        // 出错换了模型（瞬时的，看着的时候才有，`withChanges` 插进来的）：和回报一样不属于哪一轮
        if (e.body?.why !== 'failover') break;
        timeline.speak(Date.parse(e.at));
        items.push(changeNote(e));
        break;
      case 'peer.idle':
        // 别的会话空下来了、等不到了（C-6）：和回报一样不属于哪一轮
        timeline.speak(Date.parse(e.at));
        items.push(peerNote(e));
        break;
      case 'context.compacted': {
        const note = compactedNote(e, stats.get(e.seq) ?? null);
        items.push(note);
        if (turns.has(e.turn)) turns.get(e.turn).note = note;
        break;
      }
      case 'model.called':
        // 回顾这类辅助请求（带 `purpose`）不属于哪一轮，也不对上时间线的思考
        if (b.purpose) break;
        // 压缩没压成：红色实心圆点一行；手动压缩那一轮不另起「出错了」的收尾行（蓝图「压缩的进度」第 6 条）
        if (b.compaction && b.result === 'error') {
          items.push(compactFailedNote(e));
          if (turns.has(e.turn)) turns.get(e.turn).compactFailed = true;
        }
        turns.get(e.turn)?.calls.push(b);
        timeline.called(e);
        break;
      case 'session.recapped':
        // 记下它讲到的那一轮（那时最后一轮没撤销的）：那一轮撤了回顾跟着藏（蓝图「回顾」第 6 条）
        items.push({ ...recapNote(e, !!e.local), covers: started.findLast((n) => !reverted.has(n)) ?? null });
        break;
      case 'turn.ended': {
        const turn = turns.get(e.turn);
        if (!turn) break;
        turn.ended = true;
        timeline.end(e.turn, Date.parse(e.at));
        if (turn.manual && turn.compactFailed) break;
        // 压好了、清空了的那一轮：不另起收尾行；手动压缩的用时和用量接在那一行后面
        if (turn.manual && turn.note && b.reason === 'completed') {
          if (turn.note.compaction !== 'clear') turn.note.text += ` · ${seconds(Date.parse(e.at) - Date.parse(turn.start))}${usageText(turn.calls)}`;
          break;
        }
        items.push(doneItem(e, turn));
        break;
      }
      case 'turn.reverted':
        for (const n of b.turns) reverted.add(n);
        break;
      case 'turn.unreverted':
        for (const n of b.turns) reverted.delete(n);
        break;
      default:
        break;
    }
  }
  if (live) {
    hear(live.seen);
    // 这次请求里第几块这一种：和落了盘以后的编号一样
    const nth = new Map();
    live.blocks.forEach((block, k) => {
      if (!block) return;
      spoke.add(live.turn);
      const n = nth.get(block.kind) ?? 0;
      nth.set(block.kind, n + 1);
      if (block.kind === 'text') {
        timeline.speak(block.start);
        items.push({ type: 'reply', key: `rL${live.seen}-${k}`, turn: live.turn, text: block.text, streaming: !block.done });
      } else if (block.kind === 'reasoning' || block.kind === 'tool_call') {
        timeline.streaming(live.turn, live.seen, block, n);
      }
    });
  }
  const open = [...turns.entries()].find(([, v]) => !v.ended);
  // 打断的那一轮收尾：这时还有在跑的后台任务，记着几个（后面接一句「后台还有 N 个在跑」，蓝图「后台任务」第 6 条）
  const lastDone = items.findLast((it) => it.type === 'done');
  if (lastDone?.interrupted) lastDone.jobs = runningJobs(tasksOf(events));
  // 没在回答了还排着的（核心重启、崩了没接着开）：进正文，不一直挂着
  if (!open) items.push(...queue.splice(0));
  // 一轮开始、她还一块都没来：三个球（蓝图 `web.md`「她出第一个字之前」）；人要的压缩、清空单开的那一轮（没有 `trigger`）不画她的头、也不画这三个球（蓝图「压缩、清空」；照 TUI 的 `waiting()`，那一轮自己在跑）
  if (open && !spoke.has(open[0]) && !open[1].manual) items.push({ type: 'waiting', key: `w${open[0]}`, turn: open[0] });
  return {
    items: items.filter((it) => !reverted.has(it.turn) && !(it.covers != null && reverted.has(it.covers))),
    /** 排着的话：画在运行状态行下面 */
    queued: queue.filter((q) => !reverted.has(q.turn)),
    running: open ? { turn: open[0], start: Date.parse(open[1].start), level: open[1].level } : null,
    level,
  };
}

/** 图标和它后面的空：`▣` 多空一格，别的照 `layout.json`；没记着级别的用 `✻`（`tui.md`「正文」第 4 条）。 */
function icon(level) {
  const l = res.layout;
  const mark = l.level_icons[level];
  return mark ? mark + (l.done_gap[level] ?? '') : l.done_icon;
}

/** 收尾那一行：照常结束的写时刻、端点/模型、用时、本轮用量；打断、出错、别的原因各一种写法。 */
function doneItem(e, turn) {
  const base = { type: 'done', key: `d${e.seq}`, turn: e.turn, level: turn.level, tone: 'dim' };
  const reason = e.body.reason;
  if (reason === 'interrupted') return { ...base, text: icon(turn.level) + t('interrupted'), interrupted: true, jobs: 0 };
  if (reason === 'error') {
    const failure = [...turn.calls].reverse().find((c) => c.error)?.error ?? { class: 'other', message: '' };
    return { ...base, tone: 'error', text: t('failed', { message: failureText(failure) }) };
  }
  // 别的原因（重启、崩了、到了次数上限）：和被打断一样，图标加界面语言的字；不认识的照原样（蓝图「收尾那一行的数」）
  if (reason !== 'completed') {
    const said = res.text.turn_end?.[reason];
    return { ...base, text: said ? icon(turn.level) + said : reason };
  }
  const by = turn.by ?? [...turn.calls].reverse().find((c) => c.model) ?? {};
  const elapsed = seconds(Date.parse(e.at) - Date.parse(turn.start));
  const head = t('done', { time: hhmm(e.at), endpoint: by.endpoint ?? '', model: by.model ?? '', elapsed });
  return { ...base, text: icon(turn.level) + head + usageText(turn.calls) };
}

/** 本轮用量：这一轮每次请求的输入加输出，括号里是命中率；一次都没调成的不写（照 TUI 的 `turn_usage`）。 */
function usageText(calls) {
  let input = 0;
  let output = 0;
  let hit = 0;
  for (const c of calls) {
    if (!c.usage) continue;
    input += c.usage.uncached + c.usage.cache_read + c.usage.cache_write;
    output += c.usage.output;
    hit += c.usage.cache_read;
  }
  return input === 0 ? '' : t('done_usage', { tokens: short(input + output), percent: hitRate(hit, input) });
}
