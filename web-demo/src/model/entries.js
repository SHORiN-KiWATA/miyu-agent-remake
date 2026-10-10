// @ts-check
//! 条目 → 正文的几种（蓝图 `web.md`「照条目画」第 2 条；核心 9-8，`view.md`「条目」）：核心算好了显示什么，这里只换成对话区画的那几种，
//! 形状和原来照事件算的一样（`ui/chat.js`、`ui/timeline.js`、`ui/said.js`、`ui/notes.js` 不用管条目是哪来的）。纯函数。
//!
//! - `user` 是你的话：`queued` 的不进正文（画在运行状态行下面），`withdrawn` 的不画；开这一轮的那一句（这一轮第一条她的条目前面最后
//!   一条你的话）记 `opens`，编辑只在它上面。
//! - `reply` 是她的回答，`open` 的照在收的画。
//! - `group` 连它的 `thought`、`tool` 是时间线的一段：收起那一行照核心给的 `summary`（界面语言是自动的照 `summary_en`）；一步的标题照
//!   `title`（`ui/steps.js` 经 `model/words.js` 的 `row`）、改了多少行照 `diff`、用时照 `took_ms`。
//! - `end` 是收尾那一行，`notice` 是不挂在她头下的一行：字由网页写，照原来的写法（`model/notes.js`）。
//! - `hidden` 的不画；一轮在跑、这一轮还没有她的条目的，画三个球。

import { res, t } from '../util/res.js';
import { short, hitRate, seconds, hhmm } from './format.js';
import { speakerOf, peerNote, changeNote, modelNote, compactedNote, failureText, recapNote, compactFailedNote, workspaceNote } from './notes.js';

/** 权限级别：只读的写 `read_only`，别的照 `level`（`tui.md`「正文」第 4 条）。 @param {any} p */
export const levelOf = (p) => (p?.read_only ? 'read_only' : p?.level ?? 'workspace');

/** 条目编号里的序号（`m12`、`e30` → 12、30；`b6.1` → 6）：排排着的话、回报的编号用。 @param {string} id */
export const seqOf = (id) => Number.parseInt(String(id).replace(/^[a-z]+/, ''), 10) || 0;

/** `tool` 的状态：在写参数、在跑，别的都算做完了（`status` 是哪一种） */
const RUNNING = new Set(['preparing', 'running']);

/**
 * @typedef {{account?: string|null, parent?: string|null}} Options 这个页面登录的账号（自己说的话不写是谁）；这个会话是子会话的，派它的会话
 */

/**
 * 条目和会话状态 → 正文的条目、排着的话、在跑的那一轮、权限级别（同原来 `model/transcript.js` 的 `project` 交回的）。
 * @param {any[]} entries 照显示的先后
 * @param {any} status `view.status`（订阅前是 `null`）
 * @param {Options} [opts]
 */
export function itemsOf(entries, status, opts = {}) {
  const shown = entries.filter((e) => !e.hidden && !e.withdrawn);
  const byId = new Map(shown.map((e) => [e.id, e]));
  const jobs = jobsOf(entries, status);
  const items = [];
  const queued = [];
  /** 每一轮来过她的条目（不是你的话）：开轮的那一句、三个球照它 */
  const spoke = new Set();
  for (const e of shown) {
    switch (e.kind) {
      case 'user': {
        const item = userItem(e, jobs, opts);
        if (e.queued) queued.push(item);
        else items.push(item);
        break;
      }
      case 'reply':
        spoke.add(e.turn);
        items.push({ type: 'reply', key: e.id, turn: e.turn ?? null, text: e.text ?? '', streaming: !!e.open });
        break;
      case 'group':
        spoke.add(e.turn);
        items.push(segmentOf(e, byId));
        break;
      case 'end':
        items.push(endItem(e, status));
        break;
      case 'notice': {
        const note = noticeItem(e, jobs);
        if (note) items.push(note);
        break;
      }
      default:
        // `thought`、`tool` 跟着它的那一段画
        break;
    }
  }
  markOpeners(items);
  const level = status?.permission ? levelOf(status.permission) : items.findLast((it) => it.type === 'user')?.level ?? 'workspace';
  const busy = status && status.state && status.state !== 'idle';
  const turn = busy ? currentTurn(shown) : null;
  // 在压缩的不画：压缩的进度那一行已经说着在做什么（2026-10-10 项目主人：手动压缩时进度条上面多一个头像和三个球）
  const compacting = status?.doing?.what === 'compacting';
  if (busy && turn != null && !spoke.has(turn) && !compacting) items.push({ type: 'waiting', key: `w${turn}`, turn });
  return {
    items,
    queued,
    running: busy ? { turn, start: status.since ? Date.parse(status.since) : Date.now(), level } : null,
    level,
  };
}

/** 在跑的那一轮：最后一条带 `turn`、这一轮还没有 `end` 的。 @param {any[]} entries */
function currentTurn(entries) {
  const ended = new Set(entries.filter((e) => e.kind === 'end').map((e) => e.turn));
  for (let i = entries.length - 1; i >= 0; i--) {
    const n = entries[i].turn;
    if (n != null) return ended.has(n) ? null : n;
  }
  return null;
}

/**
 * 派出去的任务：编号 → 种类、标题、子会话、命令（你的话是子代理说的照它写是谁；回报那一行写标题）。照会话状态的 `jobs`，再补上条目里
 * 派它的那一步（`tool` 的 `job`、标题的 `object`）。
 * @param {any[]} entries @param {any} status
 */
function jobsOf(entries, status) {
  /** @type {Map<string, {what: string, title: string, session: string|null, command: string|null}>} */
  const jobs = new Map();
  for (const j of status?.jobs ?? []) jobs.set(j.job, { what: j.what, title: j.title ?? '', session: j.session ?? null, command: j.command ?? null });
  for (const e of entries) {
    if (e.kind === 'tool' && e.job && !jobs.has(e.job)) jobs.set(e.job, { what: e.name === 'subagent' ? 'agent' : 'command', title: e.title?.object ?? '', session: null, command: null });
  }
  return jobs;
}

/** @param {any} e @param {Map<string, any>} jobs @param {Options} opts */
function userItem(e, jobs, opts) {
  const speaker = e.from ? speakerOf(e.from, jobs, opts.parent ?? null) : { kind: 'person', account: opts.account ?? null, name: opts.account ?? '' };
  return {
    type: 'user',
    key: e.id,
    seq: seqOf(e.id),
    turn: e.turn ?? null,
    text: e.text ?? '',
    attachments: (e.attachments ?? []).map((/** @type {any} */ a) => ({ kind: a.kind, blob: a.blob, media_type: a.media_type, width: a.width ?? null, height: a.height ?? null, name: a.name ?? null })),
    speaker,
    opens: false,
    level: e.level ?? null,
  };
}

/** 开这一轮的那一句：每一轮第一条不是你的话的条目前面，最后一条这一轮的你的话。 @param {any[]} items */
function markOpeners(items) {
  /** @type {Map<number, any>} */
  const last = new Map();
  const done = new Set();
  for (const it of items) {
    if (it.turn == null || done.has(it.turn)) continue;
    if (it.type === 'user') last.set(it.turn, it);
    else {
      done.add(it.turn);
      const opener = last.get(it.turn);
      if (opener) opener.opens = true;
    }
  }
  for (const [turn, it] of last) if (!done.has(turn)) it.opens = true;
}

/**
 * 时间线的一段：照它的 `steps` 找出每一步（`thought`、`tool`），换成 `ui/steps.js` 画的那一种；收起那一行照核心给的。
 * @param {any} g @param {Map<string, any>} byId
 */
function segmentOf(g, byId) {
  const en = res.language?.summary === 'en';
  const parts = (en ? g.summary_en : g.summary) ?? g.summary ?? [];
  return {
    type: 'steps',
    key: g.id,
    turn: g.turn ?? null,
    finished: !g.open,
    steps: (g.steps ?? []).map((/** @type {string} */ id) => byId.get(id)).filter(Boolean).map(stepOf),
    summary: { failed: !!g.failed, spans: parts.map((/** @type {any} */ p) => ({ text: p.text, tone: p.tone ?? 'base' })) },
  };
}

/** 一步：思考、工具。 @param {any} e */
export function stepOf(e) {
  const start = e.at ? Date.parse(e.at) : null;
  if (e.kind === 'thought') {
    const end = !e.open && start != null && e.took_ms != null ? start + e.took_ms : null;
    return { key: e.id, kind: 'thought', text: e.text ?? '', state: e.open ? 'thinking' : 'done', start, end };
  }
  const done = !RUNNING.has(e.state);
  let parsed = null;
  try {
    parsed = e.args ? JSON.parse(e.args) : null;
  } catch {
    // 还在写的参数读不全：照原文
  }
  return {
    key: e.id,
    kind: 'tool',
    name: e.name ?? '',
    args: e.args ?? '',
    parsed,
    state: done ? 'done' : e.state,
    status: done ? e.state : null,
    output: '',
    said: null,
    callId: e.call ?? null,
    start,
    end: done && start != null && e.took_ms != null ? start + e.took_ms : null,
    duration: done ? e.took_ms ?? null : null,
    job: e.job ?? null,
    toTitle: e.to_title ?? null,
    // 核心给的（9-8）：标题那一句、改了多少行、结果里的图、确认、改了哪些文件（三补，`action` 是 `created`、`changed`、`trashed`）
    title: e.title ?? null,
    lines: e.diff ?? null,
    images: e.images ?? [],
    approval: e.approval ?? null,
    files: e.files ?? [],
  };
}

/** 图标和它后面的空：`▣` 多空一格，别的照 `layout.json`；没记着级别的用 `✻`（`tui.md`「正文」第 4 条）。 @param {string|null} level */
function icon(level) {
  const l = res.layout;
  const mark = level ? l.level_icons[level] : null;
  return mark ? mark + (l.done_gap[level ?? ''] ?? '') : l.done_icon;
}

/**
 * 收尾那一行：照常结束的写时刻、端点/模型、用时、本轮用量；打断、出错、别的原因各一种写法（同原来的 `doneItem`）。
 * @param {any} e @param {any} status
 */
function endItem(e, status) {
  const level = e.level ?? null;
  const base = { type: 'done', key: e.id, turn: e.turn ?? null, level, tone: 'dim' };
  const reason = e.reason;
  if (reason === 'interrupted') {
    // 打断的那一轮还有在跑的后台任务：后面接一句「后台仍有 N 个任务在运行」
    const jobs = (status?.jobs ?? []).filter((/** @type {any} */ j) => j.state === 'running').length;
    return { ...base, text: icon(level) + t('interrupted'), interrupted: true, jobs };
  }
  if (reason === 'error') return { ...base, tone: 'error', text: t('failed', { message: failureText(e.error ?? { class: 'other', message: '' }) }) };
  if (reason !== 'completed') {
    const said = res.text.turn_end?.[reason];
    return { ...base, text: said ? icon(level) + said : reason };
  }
  const head = t('done', { time: hhmm(e.at), endpoint: e.endpoint ?? '', model: e.model ?? '', elapsed: seconds(e.took_ms ?? 0) });
  return { ...base, text: icon(level) + head + usageText(e.usage) };
}

/** 本轮用量：输入加输出，括号里是命中率；没有的不写。 @param {any} u */
function usageText(u) {
  if (!u) return '';
  const input = (u.uncached ?? 0) + (u.cache_read ?? 0) + (u.cache_write ?? 0);
  return input === 0 ? '' : t('done_usage', { tokens: short(input + (u.output ?? 0)), percent: hitRate(u.cache_read ?? 0, input) });
}

/**
 * 旁白 → 不挂在她头下的一行（字照原来 `model/notes.js` 的写法）。撤销那一条不画（撤掉的条目本来就藏起了，提示在输入框上面）。
 * @param {any} e @param {Map<string, any>} jobs
 */
function noticeItem(e, jobs) {
  const seq = seqOf(e.id);
  const event = (/** @type {string} */ kind, /** @type {any} */ body) => ({ seq, kind, at: e.at, turn: e.turn ?? null, body });
  const keyed = (/** @type {any} */ note) => (note ? { ...note, key: e.id } : null);
  switch (e.what) {
    case 'compaction':
      if (e.state === 'failed') return keyed(compactFailedNote(event('model.called', { error: e.error ?? {} })));
      if (e.state === 'running') return null;
      // 提前在后台压好、直接换上的（核心 6-11 三补的 `prepared`）：正文里不画，起压那一刻已经弹过「已触发上下文压缩」（2026-10-10 项目主人）
      if (e.prepared) return null;
      return keyed(withTurnUsage(compactedNote(event('context.compacted', { trigger: e.trigger, instructions: e.instructions }), e.before != null ? { before: e.before, after: e.after } : null), e, true));
    case 'cleared':
      return keyed(compactedNote(event('context.compacted', { trigger: 'clear' })));
    case 'job':
      return jobNote(e, jobs);
    case 'recap':
      return keyed({ ...recapNote(event('session.recapped', { text: e.text ?? '' })), covers: e.covers ?? null });
    case 'model':
      if (e.why === 'failover') return keyed(changeNote({ ...event('model.changed', { endpoint: e.endpoint, model: e.model }), local: 0 }));
      if (e.why === 'replaced') {
        const from = [e.from?.endpoint, e.from?.model].filter(Boolean).join('/');
        const to = [e.endpoint, e.model].filter(Boolean).join('/');
        return keyed(modelNote(event('session.policy_changed', { replaced: from, model: to })));
      }
      return null;
    case 'workspace':
      return keyed(workspaceNote(event('session.workspace_changed', { cwd: e.cwd })));
    case 'peer':
      return keyed(peerNote(event('peer.idle', { session: e.session, reason: e.reason, status: e.status })));
    case 'answered':
      return answeredNote(e);
    default:
      // `reverted`（撤销：提示在输入框上面）、`paused`（暂停了自动压缩：原来也不画）这些不画
      return null;
  }
}

/** 手动压缩那一行接上用时、用量（核心把那一轮的接在这一条上）。 @param {any} note @param {any} e @param {boolean} manual */
function withTurnUsage(note, e, manual) {
  if (!note || !manual || e.trigger !== 'manual' || e.took_ms == null) return note;
  return { ...note, text: `${note.text} · ${seconds(e.took_ms)}${usageText(e.usage)}` };
}

/**
 * 后台任务了结的那一行：命令完成绿点、失败红点，停了的暗点；点开看命令和输出（后台命令）、报告（子代理）。
 * @param {any} e @param {Map<string, any>} jobs
 */
function jobNote(e, jobs) {
  const what = e.job_kind === 'agent' ? 'agent' : 'command';
  const title = e.title ?? jobs.get(e.job)?.title ?? e.job;
  const marks = res.layout.note_marks;
  const texts = res.text.notes[what];
  let tone = 'stopped';
  let text;
  if (e.mark === 'done') {
    tone = 'good';
    // 后台命令完成的接用时（核心 9-8 三补起旁白带 `took_ms`）
    text = t(`notes.${what}.done`, { title }) + (what === 'command' && e.took_ms != null ? ` · ${seconds(e.took_ms)}` : '');
  } else if (e.mark === 'failed') {
    tone = 'error';
    text = e.signal != null ? t(`notes.${what}.signal`, { title, signal: e.signal }) : e.exit_code != null ? t(`notes.${what}.failed`, { title, code: e.exit_code }) : t('notes.command.failed_plain', { title });
  } else if (texts[e.reason]) text = t(`notes.${what}.${e.reason}`, { title });
  else text = t(`notes.${what}.stopped`, { title });
  const detail = what === 'agent'
    ? { kind: 'text', text: e.report ?? '', truncated: false }
    : { kind: 'output', command: e.command ?? jobs.get(e.job)?.command ?? null, hash: e.output?.blob ?? null, chars: e.output?.chars ?? null };
  return { type: 'note', key: e.id, seq: seqOf(e.id), turn: null, tone, mark: marks[tone] ?? '', text, detail };
}

/**
 * 一组题答了：交给软件包 `asking` 画（挂载位 `chat.item`），带上问的那一组、答的（同原来的 `question.asked`、`question.answered`）。
 * @param {any} e
 */
function answeredNote(e) {
  const seq = seqOf(e.id);
  const asked = { seq, kind: 'question.asked', at: e.at, body: { call_id: e.id, questions: e.questions ?? [] } };
  const event = { seq, kind: 'question.answered', at: e.at, by: e.by ?? null, body: { call_id: e.id, answers: e.answers ?? [], by: e.by ?? null } };
  return { type: 'note', key: e.id, seq, turn: e.turn ?? null, slot: 'asking', event, asked };
}
