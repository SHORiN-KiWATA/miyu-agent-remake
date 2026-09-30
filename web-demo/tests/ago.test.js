// @ts-check
//! 什么时候开的（蓝图 `web.md`「全部会话」第 2 条）：时刻照会话编号（UUIDv7 的前 48 位是毫秒），写成「刚才」「N 分钟前」……

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadRes } from './support.js';
import { ago, uuidTime } from '../src/model/ago.js';

loadRes();

test('UUIDv7 的前 48 位是造它的那一刻（毫秒）；不是 v7 的交 null', () => {
  assert.equal(uuidTime('0199a000-0000-7000-8000-000000000001'), 0x0199a0000000);
  assert.equal(uuidTime('not-a-uuid'), null);
});

test('今天的写刚才、几分钟前、几小时前；这一周的写几天前；更早的写几月几日', () => {
  const now = new Date(2026, 8, 30, 18, 0).getTime();
  const at = (d, h, m) => new Date(2026, 8, d, h, m).getTime();
  assert.equal(ago(at(30, 17, 59) + 30000, now), '刚才');
  assert.equal(ago(at(30, 17, 45), now), '15 分钟前');
  assert.equal(ago(at(30, 9, 0), now), '9 小时前');
  assert.equal(ago(at(28, 12, 0), now), '2 天前');
  assert.equal(ago(at(20, 12, 0), now), '9 月 20 日');
  assert.equal(ago(new Date(2025, 11, 3).getTime(), now), '2025 年 12 月 3 日');
});
