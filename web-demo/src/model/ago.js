// @ts-check
//! 什么时候开的（蓝图 `web.md`「全部会话」第 2 条）：时刻照会话编号（UUIDv7 的前 48 位是造它的那一刻，毫秒）；今天的写「刚才」
//! 「N 分钟前」「N 小时前」，这一周的写「N 天前」，今年更早的写「M 月 D 日」，别的年份带年。字在 `text/zh.json` 的 `ago`。纯函数。

import { t } from '../util/res.js';

/** UUIDv7 里造它的那一刻（毫秒）；不是这个写法的交 `null`。 */
export function uuidTime(id) {
  const hex = String(id).replace(/-/g, '');
  if (!/^[0-9a-f]{32}$/i.test(hex) || hex[12] !== '7') return null;
  return parseInt(hex.slice(0, 12), 16);
}

/** 本地时间的那一天从几点开始（毫秒） */
const dayStart = (ms) => {
  const d = new Date(ms);
  return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
};

/**
 * @param {number} at 那一刻
 * @param {number} now 现在
 */
export function ago(at, now) {
  const secs = Math.max(0, (now - at) / 1000);
  if (secs < 60) return t('ago.now');
  if (secs < 3600) return t('ago.minutes', { n: Math.floor(secs / 60) });
  const days = Math.round((dayStart(now) - dayStart(at)) / 86400000);
  if (days === 0) return t('ago.hours', { n: Math.floor(secs / 3600) });
  if (days < 7) return t('ago.days', { n: days });
  const d = new Date(at);
  if (d.getFullYear() === new Date(now).getFullYear()) return t('ago.date', { m: d.getMonth() + 1, d: d.getDate() });
  return t('ago.full', { y: d.getFullYear(), m: d.getMonth() + 1, d: d.getDate() });
}
