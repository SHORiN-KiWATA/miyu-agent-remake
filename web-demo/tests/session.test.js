// @ts-check
//! 会话的先后、打开页面时进哪个会话（蓝图 `web.md`「左栏」「连核心」第 4 条）。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { rank, startupSession } from '../src/model/session.js';


test('先后：置顶的在最前，别的照最近活动从近到远；不知道活动时刻的照开的时刻（编号里的时间）', () => {
  // 编号里的时间：前 12 位十六进制是毫秒
  const id = (ms) => `${ms.toString(16).padStart(12, '0').replace(/^(.{8})(.{4})$/, '$1-$2')}-7000-8000-000000000000`;
  const rows = [
    { session: id(1000), pinned: false, active: 9000 },
    { session: id(2000), pinned: false, active: null },
    { session: id(3000), pinned: true, active: 4000 },
    { session: id(5000), pinned: false, active: null },
    { session: id(6000), pinned: true, active: 8000 },
  ];
  assert.deepEqual(rank(rows).map((r) => r.session), [id(6000), id(3000), id(1000), id(5000), id(2000)]);
});

test('打开页面时进哪个会话：ui.startup 是 recent 的进最近的那个，new、读不出来的（核心旧、没有这一项）是新会话', () => {
  const recent = { items: { 'ui.startup': { value: 'recent', origin: { layer: 'personal' } } } };
  assert.equal(startupSession(recent, ['b', 'a']), 'b');
  assert.equal(startupSession(recent, []), null, '一个会话都没有的是新会话');
  assert.equal(startupSession({ items: { 'ui.startup': { value: 'new' } } }, ['b']), null);
  assert.equal(startupSession(null, ['b']), null, '读不出来（拒了）的照出厂的 new');
  assert.equal(startupSession({ items: {} }, ['b']), null);
});
