// @ts-check
//! 会话列表（蓝图 `web.md`「会话列表」，`/sessions` 打开）：排哪几行、什么先后、搜哪些字、左边那一格是什么记号。纯函数。

import { rank } from './session.js';
import { shortSession } from './words.js';

/**
 * @typedef {{session: string, title: string|null, pinned: boolean, active: number|null}} Row 全部顶层会话里的一个（照「全部会话」读）
 * @typedef {{session: string, short: string, title: string|null, active: number|null, mark: 'running'|'unread'|null,
 *   hit: {title: [number, number][], short: [number, number][]}}} Item 列表里的一行
 */

/**
 * 一段字里对上 `query` 的几处（不分大小写，一处处往后找、不重叠）：`[开头, 结尾)`。
 * @param {string|null} text
 * @param {string} query
 * @returns {[number, number][]}
 */
export function hits(text, query) {
  if (!text || !query) return [];
  const low = text.toLowerCase();
  const q = query.toLowerCase();
  /** @type {[number, number][]} */
  const out = [];
  for (let at = low.indexOf(q); at >= 0; at = low.indexOf(q, at + q.length)) out.push([at, at + q.length]);
  return out;
}

/**
 * 列表的几行：当前会话在最上面，别的照左栏（`rank`）；搜着的只留标题或短编号对得上的。
 * @param {Row[]} rows
 * @param {string|null} current 正在看的会话（还没开的新会话是 `null`）
 * @param {string} query 搜索框里的字（只有空白的当没搜）
 * @param {(session: string) => {running: boolean, unread: boolean}} live 在不在跑、跑完了有没有看过
 * @returns {Item[]}
 */
export function sessionList(rows, current, query, live) {
  const q = query.trim();
  const mine = rows.find((r) => r.session === current);
  const ordered = [...(mine ? [mine] : []), ...rank(rows.filter((r) => r !== mine))];
  return ordered.flatMap((r) => {
    const short = shortSession(r.session);
    const hit = { title: hits(r.title, q), short: hits(short, q) };
    if (q && !hit.title.length && !hit.short.length) return [];
    const state = live(r.session);
    const mark = state.running ? 'running' : state.unread && r.session !== current ? 'unread' : null;
    return [{ session: r.session, short, title: r.title, active: r.active, mark, hit }];
  });
}
