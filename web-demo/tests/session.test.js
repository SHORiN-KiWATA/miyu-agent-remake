// @ts-check
//! 左栏的一项从日志推：标题、在不在跑（蓝图 `web.md`「会话表的一项」）。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadRes, sampleLog } from './support.js';
import { summarize } from '../src/model/session.js';

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
