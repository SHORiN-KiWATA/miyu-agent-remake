// @ts-check
//! 左栏的一项，从这个会话的事件推出来：标题、在不在跑、什么时候开的（蓝图 `web.md`「会话表的一项」）。
//!
//! 核心的 `session.list` 现在只给编号和是不是一次性的，会话列表流还没有（`04-核心协议.md` 第五节、
//! 蓝图 `protocol.md`）；先在头里从日志推，核心有了列表流只换数据源。置顶、最近活动也从日志推，左栏照它们排（`rank`）。

import { uuidTime } from './ago.js';

/**
 * @param {string} id 会话编号
 * @param {any[]} events 这个会话的持久事件，照序号
 * @returns {{session: string, title: string|null, running: boolean, created: string|null, pinned: boolean, active: number|null}}
 */
export function summarize(id, events) {
  let title = null;
  let first = null;
  let running = false;
  let pinned = false;
  for (const e of events) {
    if (e.kind === 'session.meta_changed' && typeof e.body.pinned === 'boolean') pinned = e.body.pinned;
    // 去掉标题写成空的（`kernel/events-bodies.md`）：回到照第一句话写
    if (e.kind === 'session.meta_changed' && typeof e.body.title === 'string') title = e.body.title || null;
    if (e.kind === 'message.user' && first == null) first = text(e);
    if (e.kind === 'turn.started') running = true;
    if (e.kind === 'turn.ended') running = false;
  }
  // 没起名字的拿第一句话顶：换行换成空格，一行里放不下由界面截掉
  const fallback = first ? first.replace(/\s+/g, ' ').trim() : '';
  // 最近活动：日志最后一条的时刻（和核心 C-3 的 `last_active` 同一个意思），左栏照它排
  const last = events.at(-1)?.at;
  return { session: id, title: title ?? (fallback || null), running, created: events[0]?.at ?? null, pinned, active: last ? Date.parse(last) : null };
}

/**
 * 会话的先后（蓝图 `web.md`「左栏」组头、「全部会话」第 3 条）：置顶的在最前，别的照最近活动从近到远；不知道活动时刻的（没读过日志、
 * 核心还没给 `last_active`）照开的时刻（编号里的时间）。
 * @template {{session: string, pinned?: boolean, active?: number|null}} T
 * @param {T[]} rows
 * @returns {T[]}
 */
export function rank(rows) {
  const at = (r) => r.active ?? uuidTime(r.session) ?? 0;
  return [...rows].sort((a, b) => Number(!!b.pinned) - Number(!!a.pinned) || at(b) - at(a));
}

/** 一条 `message.user` 里的字：文字块接起来（现在经协议发来的只有一块文字，蓝图 `protocol.md`）。 */
export function text(e) {
  return (e.body.blocks ?? []).filter((b) => b.type === 'text').map((b) => b.text).join('\n');
}

/**
 * 一条消息里的附件（图片、文件块，`kernel/blocks.md`），照先后；格都写上，没有的是 `null`（蓝图 `web.md`「附件」第 6 条）。
 * @returns {{kind: 'image'|'file', blob: string, media_type: string, width: number|null, height: number|null, name: string|null}[]}
 */
export function attachments(e) {
  return (e.body.blocks ?? []).filter((b) => b.type === 'image' || b.type === 'file').map((b) => ({
    kind: b.type,
    blob: b.blob,
    media_type: b.media_type,
    width: b.width ?? null,
    height: b.height ?? null,
    name: b.name ?? null,
  }));
}
