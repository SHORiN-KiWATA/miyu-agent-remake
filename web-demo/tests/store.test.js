// @ts-check
//! 在收的那一次回复：`model.delta` 攒成一块块，记下每一块什么时候开始、收全（蓝图 `web.md`「时间线的数」第 2、4 条）。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadRes, ev, ms } from './support.js';
import { Store, emptySession } from '../src/core/store.js';

loadRes();

const MODEL = { kind: 'model', endpoint: 'deepseek', model: 'deepseek-v4' };

/** 一个只收推送的仓库，里面一个会话 `S`。 */
function fresh() {
  const store = new Store(/** @type {any} */ ({ onPush() {}, request: async () => ({}) }));
  store.sessions.set('S', emptySession('S'));
  return store;
}

/** 一段增量（瞬时的，没有序号）。 */
function delta(at, body) {
  const { seq, ...e } = ev(0, at, 'model.delta', 3, { seen: 3, ...body }, MODEL);
  return e;
}

const push = (store, event) => store.push('event', { session: 'S', event });

test('一块开始、接字、收全；同一次请求的下一块开始了，前面的算收全', () => {
  const store = fresh();
  push(store, delta(1, { index: 0, start: 'reasoning' }));
  push(store, delta(1.5, { index: 0, text: '想' }));
  push(store, delta(2, { index: 1, start: 'tool_call', name: 'shell' }));
  push(store, delta(2.5, { index: 1, text: '{"command":"ls"}' }));
  const s = store.sessions.get('S');
  assert.deepEqual(s.live.blocks.map((b) => [b.kind, b.name ?? null, b.text, b.done, b.start, b.end]), [
    ['reasoning', null, '想', true, ms(1), ms(2)],
    ['tool_call', 'shell', '{"command":"ls"}', false, ms(2), null],
  ]);
  push(store, delta(3, { index: 1, end: true }));
  assert.deepEqual([s.live.blocks[1].done, s.live.blocks[1].end], [true, ms(3)]);
  assert.deepEqual(s.marks.get('3:reasoning:0'), { start: ms(1), end: ms(2) });
  assert.deepEqual(s.marks.get('3:tool_call:0'), { start: ms(2), end: ms(3) });
});

test('回复落了盘，在收的扔掉，记下的时刻留着；还开着的停在落盘那一刻', () => {
  const store = fresh();
  push(store, delta(1, { index: 0, start: 'reasoning' }));
  push(store, ev(1, 4, 'message.assistant', 3, { seen: 3, blocks: [{ type: 'reasoning', text: '想' }] }, MODEL));
  const s = store.sessions.get('S');
  assert.equal(s.live, null);
  assert.deepEqual(s.marks.get('3:reasoning:0'), { start: ms(1), end: ms(4) });
});

test('一轮结束：在收的扔掉，还开着的停在结束那一刻', () => {
  const store = fresh();
  push(store, delta(1, { index: 0, start: 'reasoning' }));
  push(store, ev(1, 6, 'turn.ended', 3, { reason: 'interrupted' }));
  const s = store.sessions.get('S');
  assert.equal(s.live, null);
  assert.deepEqual(s.marks.get('3:reasoning:0'), { start: ms(1), end: ms(6) });
});

test('压缩的进度（瞬时的 compaction.progress、compaction.done）：记下写了多少、估计多少；压好了记前后的用量，接着落盘的那一条 context.compacted 记上它', () => {
  const store = fresh();
  const s = store.sessions.get('S');
  const transient = (at, kind, body) => { const { seq, ...e } = ev(0, at, kind, 9, body, { kind: 'kernel' }); return e; };
  push(store, transient(1, 'compaction.progress', { seen: 8, written: 0, expected: 20000 }));
  assert.deepEqual([s.compacting.written, s.compacting.expected, s.compacting.done], [0, 20000, null]);
  push(store, transient(2, 'compaction.progress', { seen: 8, written: 3120, expected: 20000 }));
  assert.equal(s.compacting.written, 3120);
  push(store, transient(3, 'compaction.done', { seen: 8, trigger: 'auto', before: 812345, after: 31020 }));
  assert.deepEqual(s.compacting.done, { before: 812345, after: 31020 });
  push(store, ev(20, 3, 'context.compacted', 9, { upto: 8, summary: '摘要', trigger: 'auto' }));
  assert.equal(s.compacting.note, 20, '走满以前这一条先不画');
  assert.deepEqual(s.compactStats.get(20), { before: 812345, after: 31020 });
  store.finishCompaction('S');
  assert.equal(s.compacting, null);
});

test('压好了的两条谁先到不一定：落了盘的 context.compacted 先到，跟着来的 compaction.done 照样记上前后的用量', () => {
  const store = fresh();
  const s = store.sessions.get('S');
  const transient = (at, kind, body) => { const { seq, ...e } = ev(0, at, kind, 9, body, { kind: 'kernel' }); return e; };
  push(store, transient(1, 'compaction.progress', { seen: 8, written: 3120, expected: 20000 }));
  push(store, ev(20, 3, 'context.compacted', 9, { upto: 8, summary: '摘要', trigger: 'auto' }));
  assert.equal(s.compactStats.size, 0);
  push(store, transient(3, 'compaction.done', { seen: 8, trigger: 'auto', before: 812345, after: 31020 }));
  assert.deepEqual(s.compactStats.get(20), { before: 812345, after: 31020 });
  assert.equal(s.compacting.note, 20);
});

test('压缩没压成（落了盘的 model.called 带 compaction、出错）、这一轮先结束了：进度那一行收掉', () => {
  const store = fresh();
  const s = store.sessions.get('S');
  const { seq, ...progress } = ev(0, 1, 'compaction.progress', 9, { seen: 8, written: 10, expected: 20000 }, { kind: 'kernel' });
  push(store, progress);
  push(store, ev(20, 2, 'model.called', 9, { seen: 8, result: 'error', compaction: true, error: { class: 'other', message: 'boom' } }));
  assert.equal(s.compacting, null);
  push(store, progress);
  push(store, ev(21, 3, 'turn.ended', 9, { reason: 'aborted' }));
  assert.equal(s.compacting, null);
});

test('读一个会话、掉了队补上：订阅带 after（0 从头，掉队的带最后看到的序号），核心补推的事件照序号接上、去重；不再找桥读日志（2026-10-01）', async () => {
  const calls = [];
  /** @type {any} */
  let store;
  const conn = {
    onPush() {},
    request: async (method, params) => {
      calls.push([method, params.after]);
      if (method !== 'subscribe') return {};
      // 核心先补推，再回应
      const from = params.after + 1;
      for (let seq = from; seq <= 3; seq++) store.push('event', { session: 'S', event: seq === 3 ? ev(3, 3, 'turn.ended', 2, { reason: 'completed' }) : ev(seq, seq, 'message.user', undefined, { blocks: [] }) });
      return { limits: { window: 1000 }, upto: 3 };
    },
  };
  store = new Store(/** @type {any} */ (conn));
  await store.load('S');
  const s = store.sessions.get('S');
  assert.deepEqual(s.events.map((e) => e.seq), [1, 2, 3]);
  assert.deepEqual(s.limits, { window: 1000 });
  assert.equal(s.unread, false, '补的是历史：一轮结束不记成没看过');
  await store.catchUp(s);
  assert.deepEqual(s.events.map((e) => e.seq), [1, 2, 3], '补回来的重复的去掉');
  assert.deepEqual(calls, [['subscribe', 0], ['subscribe', 3]]);
});

test('起来时读最近活动的那几个（session.list 的 last_active，C-3）；旧核心没有这一格的照列出来的先后（2026-10-01）', async () => {
  const listed = [
    { session: 'new', oneshot: false, parent: null, last_active: '2026-10-01T01:00:00.000Z' },
    { session: 'old-but-busy', oneshot: false, parent: null, last_active: '2026-10-01T05:00:00.000Z' },
    { session: 'mid', oneshot: false, parent: null, last_active: '2026-10-01T03:00:00.000Z' },
    { session: 'child', oneshot: false, parent: 'mid', last_active: '2026-10-01T09:00:00.000Z' },
  ];
  const loaded = [];
  const conn = {
    onPush() {},
    request: async (method, params) => {
      if (method === 'session.list') return { sessions: listed };
      if (method === 'subscribe') loaded.push(params.session);
      return {};
    },
  };
  const store = new Store(/** @type {any} */ (conn));
  await store.boot();
  assert.deepEqual(loaded, ['old-but-busy', 'mid', 'new'], '照最近活动，子代理的不列');
  assert.deepEqual(store.order, ['old-but-busy', 'mid', 'new']);
});
