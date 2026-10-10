// @ts-check
//! 正文里不是你说的、不挂在她头下的几样（蓝图 `web.md`「不是你说的话」「后台命令、子代理的回报」「压缩、清空」）：
//! 谁说的、后台命令和子代理的回报那一行、压缩和清空那一行，和收尾那一行出错时那一句。纯函数，字在 `text/zh.json` 的 `notes`，
//! 记号在 `layout.json` 的 `note_marks`。

import { res, t } from '../util/res.js';
import { short } from './format.js';
import { shortSession } from './words.js';

/**
 * @typedef {{what: string, title: string, session: string|null, command: string|null}} Job 派出去的一个任务（会话状态的任务表、
 *   后台任务的旁白；`model/entries.js` 的 `jobsOf`）
 * @typedef {{kind: string, account: string|null, name: string}} Speaker 一句话是谁说的
 * @typedef {{kind: 'output', command: string|null, hash: string|null, chars: number|null}|{kind: 'text', text: string, truncated: boolean}} Detail
 *   点开一行看什么：后台命令的命令和整份输出（blob），子代理交回的正文（Markdown）
 */

/** 内核自己查出来的几种错：不是供应商的原话，写分类的人话（`tui.md`「正文」第 4 条）。 */
const KERNEL_CLASSES = ['bad_stream', 'empty_reply', 'bad_summary', 'compaction_paused', 'no_model', 'cooling'];

/**
 * 一句话是谁说的：人照账号；子代理照派它的那次的标题；父会话写「派它的会话」，别的会话写「从会话 短编号 收到消息」；别的 harness 照它报的名字、注明是别的 agent；平台上的人写「外部」。
 * @param {any} by 事件的 `by`
 * @param {Map<string, Job>} jobs
 * @param {string|null} [parent] 这个会话是子会话的：派它的那个会话（`session.created` 的 `parent`）
 * @returns {Speaker}
 */
export function speakerOf(by, jobs, parent = null) {
  const n = res.text.notes;
  if (by?.kind === 'person') return { kind: 'person', account: by.account ?? null, name: by.account ?? '' };
  if (by?.kind === 'session') {
    const job = [...jobs.values()].find((j) => j.session === by.id);
    if (job) return { kind: 'agent', account: null, name: t('notes.agent_speaker', { title: job.title }) };
    if (by.id === parent) return { kind: 'parent', account: null, name: n.parent_speaker, id: by.id };
    // 别的会话发来的（施工 C-5）：「从会话 短编号 收到消息」，标题界面那边照会话表接（2026-10-01 项目主人定，照终端）
    return { kind: 'session', account: null, name: t('notes.session_speaker', { id: shortSession(String(by.id ?? '')) }), id: by.id };
  }
  if (by?.kind === 'harness') return { kind: 'harness', account: null, name: t('notes.harness_speaker', { name: by.name ?? '' }) };
  if (by?.kind === 'external') return { kind: 'external', account: null, name: n.external };
  return { kind: by?.kind ?? 'unknown', account: null, name: by?.kind ?? '' };
}

/**
 * 别的会话空下来了、等不到了（`peer.idle`，跨会话施工 C-6）：不属于哪一轮。`idle` 写「会话 短编号 回复：status」（没有 status 的写「回复了」），绿点；
 * `expired`、`gone` 暗点（只记下、不叫醒她）；点开看完整的编号和 `status`。
 * @param {any} e
 */
export function peerNote(e) {
  const b = e.body;
  const id = String(b.session ?? '');
  const short = shortSession(id);
  const marks = res.layout.note_marks;
  const status = typeof b.status === 'string' ? b.status.trim() : '';
  let tone = 'dim';
  let text;
  if (b.reason === 'idle') {
    tone = 'good';
    text = status ? t('notes.peer.idle_status', { id: short, status }) : t('notes.peer.idle', { id: short });
  } else if (b.reason === 'expired' || b.reason === 'gone') {
    tone = 'stopped';
    text = t(`notes.peer.${b.reason}`, { id: short });
  } else {
    text = t('notes.peer.unknown', { id: short, reason: b.reason });
  }
  /** @type {Detail} */
  const detail = { kind: 'text', text: status ? `${id}\n\n${status}` : id, truncated: false };
  return { type: 'note', key: `n${e.seq}`, seq: e.seq, turn: null, tone, mark: marks[tone] ?? '', text, detail };
}

/** 压缩、清空那一行（`context.compacted`）：清空的绿点「上下文已清空」，别的「上下文已压缩」，附了要求的接上。 */
export function compactedNote(e, stats = null) {
  const b = e.body;
  const clear = b.trigger === 'clear';
  const ask = !clear && b.instructions ? t('notes.instructions', { text: b.instructions.replace(/\s+/g, ' ').trim() }) : '';
  // 看着压好的那一次带前后的用量（瞬时的 `compaction.done`，蓝图「压缩的进度」第 5 条）
  const head = clear ? t('notes.cleared') : stats ? t('notes.compacted_stats', { before: short(stats.before), after: short(stats.after) }) : t('notes.compacted');
  return {
    type: 'note', key: `n${e.seq}`, seq: e.seq, turn: e.turn ?? null, tone: 'good', mark: res.layout.note_marks.good,
    text: head + ask, compaction: clear ? 'clear' : (b.trigger ?? 'auto'),
    // 点开看摘要（照回报点开的 Markdown 那一种）；清空、摘要空的不能点
    detail: !clear && b.summary?.trim() ? { kind: 'text', text: b.summary, truncated: false } : null,
  };
}

/**
 * 回顾那一块（`session.recapped`，蓝图 `web.md`「回顾」第 2 条）：不挂在她的头下面，不属于哪一轮；字照原样，第一行「回顾：」由画的一方写。
 * `local` 是回应里 `cached` 为真、照回应再画一次的（第 3 条），编号另起不和日志里的撞。
 * @param {{seq: number, body: {text: string}}} e
 * @param {boolean} [local]
 */
export function recapNote(e, local = false) {
  return { type: 'note', key: `${local ? 'r' : 'n'}${e.seq}`, seq: e.seq, turn: null, tone: 'dim', mark: null, text: '', recap: e.body.text, detail: null };
}

/**
 * 压缩没压成（落了盘的 `model.called` 带 `compaction`、出错，蓝图「压缩的进度」第 6 条）：红色实心圆点一行，原因照收尾行「出错了」的写法。
 * @param {{seq: number, turn?: number, body: {error?: any}}} e
 */
export function compactFailedNote(e) {
  return {
    type: 'note', key: `n${e.seq}`, seq: e.seq, turn: e.turn ?? null, tone: 'failed', mark: res.layout.note_marks.failed,
    text: t('notes.compact_failed', { reason: failureText(e.body.error ?? {}) }), detail: null,
  };
}

/**
 * 钉着的模型没了、核心退回默认的那一行（`session.policy_changed` 带 `model`、`replaced`，核心施工 8-10）：「X 没了，换回 Y」。
 * 暗点，不属于哪一轮。
 * @param {any} e
 */
export function modelNote(e) {
  const b = e.body;
  const text = t('notes.model_replaced', { from: b.replaced, to: b.model });
  return { type: 'note', key: `n${e.seq}`, seq: e.seq, turn: null, tone: 'stopped', mark: res.layout.note_marks.stopped ?? '', text };
}

/**
 * 换了工作区那一行（蓝图「人格、预设、工作区」第 5 条，2026-10-08 项目主人定要）：「工作区：路径」，暗点；不属于哪一轮，哪个头换的都画。
 * 路径照原样带在 `path` 上，家目录写成 `~` 由画的那一头做（照时间线一步里的路径，`ui/steps.js`）。
 * @param {any} e `session.workspace_changed`
 */
export function workspaceNote(e) {
  const path = String(e.body?.cwd ?? '');
  return { type: 'note', key: `w${e.seq}`, seq: e.seq, turn: null, tone: 'stopped', mark: res.layout.note_marks.stopped ?? '', text: t('notes.workspace', { path }), path };
}

/**
 * 换了模型那一行（蓝图「后台命令、子代理的回报」那张表）：「换到 端点/模型：原来的出错了」，暗点；不属于哪一轮。
 * @param {any} e `withChanges` 插进来的
 */
export function changeNote(e) {
  const b = e.body;
  const to = [b.endpoint, b.model].filter(Boolean).join('/') || b.ref || '';
  return { type: 'note', key: `m${e.seq}-${e.local}`, seq: e.seq, turn: null, tone: 'stopped', mark: res.layout.note_marks.stopped ?? '', text: t('notes.changed', { to }) };
}

/**
 * 出错那一句（收尾那一行的「出错了：…」）：供应商的原话照写；429、402、404 前面加一句人话（以前的日志没有状态码，限速的当 429）；
 * 内核自己查出来的写分类的人话，有原话的接后面；什么都没有的写分类。
 * @param {{class?: string, message?: string, status?: number}} f `model.called` 的 `error`
 */
export function failureText(f) {
  const classes = res.text.error_classes;
  const message = (f.message ?? '').trim();
  const cls = f.class ?? 'other';
  if (KERNEL_CLASSES.includes(cls)) return message ? t('reason_with', { head: classes[cls] ?? cls, message }) : classes[cls] ?? cls;
  const status = f.status ?? (cls === 'rate_limited' ? 429 : null);
  const hint = status != null ? res.text.status_hints[String(status)] : null;
  if (hint) return message ? t('reason_with', { head: hint, message }) : hint;
  return message || classes[cls] || cls;
}
