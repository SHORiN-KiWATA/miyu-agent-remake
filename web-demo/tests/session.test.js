// @ts-check
//! 左栏的一项从日志推：标题、在不在跑（蓝图 `web.md`「会话表的一项」）。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadRes, sampleLog, ev } from './support.js';
import { summarize, rank } from '../src/model/session.js';

loadRes();

test('标题照 session.meta_changed；没在跑', () => {
  const s = summarize('s1', sampleLog());
  assert.equal(s.title, '整理 src 目录');
  assert.equal(s.running, false);
  assert.equal(s.created, '2026-09-25T07:00:00.000Z');
});

test('还没起名的拿第一句话顶；turn.started 以后、turn.ended 以前是在跑', () => {
  const s = summarize('s1', sampleLog(44));
  assert.equal(s.title, '看看 src 目录');
  assert.equal(s.running, true);
});

test('什么都还没说的没有标题', () => {
  const s = summarize('s1', sampleLog(1));
  assert.equal(s.title, null);
  assert.equal(s.running, false);
});

test('改过标题的照最后一次改的；去掉标题（写成空的）回到照第一句话写', async () => {
  const { summarize } = await import('../src/model/session.js');
  const base = [
    { seq: 1, at: '2026-09-30T00:00:00Z', kind: 'session.created', body: {} },
    { seq: 2, at: '2026-09-30T00:00:01Z', kind: 'message.user', body: { blocks: [{ type: 'text', text: '整理一下' }] } },
  ];
  const named = [...base, { seq: 3, at: '2026-09-30T00:00:02Z', kind: 'session.meta_changed', body: { title: '周报' } }];
  assert.equal(summarize('s', named).title, '周报');
  assert.equal(summarize('s', [...named, { seq: 4, at: '2026-09-30T00:00:03Z', kind: 'session.meta_changed', body: { title: '' } }]).title, '整理一下');
});

test('左栏的一项：置顶（session.meta_changed 的 pinned，取消了的不算）、最近活动（日志最后一条的时刻）', () => {
  const log = [
    ev(1, 0, 'session.created', undefined, {}),
    ev(2, 5, 'session.meta_changed', undefined, { pinned: true }),
    ev(3, 9, 'message.user', undefined, { blocks: [{ type: 'text', text: '你好' }] }),
  ];
  const s = summarize('s1', log);
  assert.equal(s.pinned, true);
  assert.equal(s.active, Date.parse(log[2].at));
  assert.equal(summarize('s1', [...log, ev(4, 12, 'session.meta_changed', undefined, { pinned: false })]).pinned, false);
  assert.equal(summarize('s1', []).active, null);
});

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
