// @ts-check
//! 会话列表（蓝图 `web.md`「会话列表」）：当前会话在最上面，别的照左栏；标题、短编号都搜；记号是在跑、跑完了没看过。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadRes } from './support.js';
import { sessionList, hits } from '../src/model/session-list.js';

loadRes();

const id = (tail) => `01a0f8fb-0000-7000-8000-0000${tail}`;
const ROWS = [
  { session: id('aaaa1111'), title: '旧的', pinned: false, active: 100 },
  { session: id('bbbb2222'), title: '置顶的 README', pinned: true, active: 50 },
  { session: id('cccc3333'), title: '新的 readme', pinned: false, active: 300 },
  { session: id('dddd4444'), title: null, pinned: false, active: 200 },
];
const live = (s) => ({ running: s === id('cccc3333'), unread: s === id('aaaa1111') || s === id('dddd4444') });

test('先后：当前会话在最上面，别的照左栏（置顶的在前，再照最近活动）', () => {
  const got = sessionList(ROWS, id('aaaa1111'), '', live);
  assert.deepEqual(got.map((r) => r.short), ['aaaa1111', 'bbbb2222', 'cccc3333', 'dddd4444']);
  assert.deepEqual(sessionList(ROWS, null, '', live).map((r) => r.short), ['bbbb2222', 'cccc3333', 'dddd4444', 'aaaa1111'], '还没开的新会话：没有当前的');
});

test('记号：在跑的转圈；跑完了没看过的圆点（当前这个不算没看过）；别的没有', () => {
  const got = sessionList(ROWS, id('aaaa1111'), '', live);
  assert.deepEqual(got.map((r) => r.mark), [null, null, 'running', 'unread']);
});

test('搜：标题、短编号都搜，不分大小写；对上的位置记下来（高亮用）', () => {
  const got = sessionList(ROWS, id('aaaa1111'), 'readme', live);
  assert.deepEqual(got.map((r) => r.short), ['bbbb2222', 'cccc3333'], '搜的时候也照先后，当前的没对上就不在');
  assert.deepEqual(got[0].hit, { title: [[4, 10]], short: [] });
  const byId = sessionList(ROWS, id('aaaa1111'), 'D44', live);
  assert.deepEqual(byId.map((r) => [r.short, r.hit.short]), [['dddd4444', [[3, 6]]]]);
  assert.deepEqual(sessionList(ROWS, null, '  ', live).length, 4, '只有空白的当没搜');
  assert.deepEqual(sessionList(ROWS, null, '没有', live), []);
});

test('对上的位置：一处处找，不重叠，不分大小写', () => {
  assert.deepEqual(hits('abcABCabc', 'bc'), [[1, 3], [4, 6], [7, 9]]);
  assert.deepEqual(hits('aaaa', 'aa'), [[0, 2], [2, 4]]);
  assert.deepEqual(hits('x', ''), []);
  assert.deepEqual(hits(null, 'a'), []);
});
