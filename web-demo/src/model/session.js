// @ts-check
//! 会话的先后、打开页面时进哪个会话（蓝图 `web.md`「左栏」「连核心」第 4 条）。左栏的一项照核心的会话表（9-5，
//! `core/session-index.js`）、读进来了的会话状态，由 `Store.summary` 合起来；置顶、最近活动也照它，左栏照它们排（`rank`）。

import { uuidTime } from './ago.js';

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

/**
 * 打开页面时进哪个会话（蓝图「连核心」第 4 条）：共用的配置项 `ui.startup` 是 `recent` 的进最近动静的那个（一个都没有的是新会话），
 * 别的（出厂的 `new`、读不出来的）是一个空的新会话。
 * @param {any} reply `config.get {keys: ["ui.startup"]}` 的回应；读不出来的是 `null`
 * @param {string[]} ranked 会话照最近动静排好的
 * @returns {string|null} 会话编号，`null` 是新会话
 */
export function startupSession(reply, ranked) {
  return reply?.items?.['ui.startup']?.value === 'recent' ? ranked[0] ?? null : null;
}
