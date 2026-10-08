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

test('出错换了模型（瞬时的 model.changed，8-9）：限额跟着换；记下这一条和收到时最后一条落了盘的序号；重试带 failover 的记上', () => {
  const store = fresh();
  const s = store.sessions.get('S');
  s.limits = { window: 300000, compaction_line: 250000 };
  push(store, ev(5, 1, 'turn.started', 5, { trigger: 4 }));
  const transient = (at, kind, body) => { const { seq, ...e } = ev(0, at, kind, 5, body, { kind: 'kernel' }); return e; };
  push(store, transient(2, 'status', { seen: 5, retry: { attempt: 1, limit: 5, wait_ms: 0, class: 'rate_limited', message: '429', failover: true } }));
  assert.equal(s.retry?.failover, true);
  assert.equal(s.retry?.due, ms(2), '换端点当场再来：等 0');
  push(store, transient(4, 'status', { seen: 5, retry: { attempt: 2, limit: 5, wait_ms: 3000, class: 'server', message: '524' } }));
  assert.equal(s.retry?.due, ms(4) + 3000, '什么时候重试：这条事件的时刻加上 wait_ms');
  push(store, transient(3, 'model.changed', { ref: '@duo', endpoint: 'bigmodel', model: 'glm-5.3-flash', limits: { window: 200000 }, why: 'failover' }));
  assert.deepEqual(s.limits, { window: 200000, compaction_line: 250000 });
  assert.equal(s.changes.length, 1);
  assert.equal(s.changes[0].after, 5);
  assert.equal(s.changes[0].body.model, 'glm-5.3-flash');
});

test('会话接下来请求的模型（8-10）：model.changed 带 endpoint、model 的换上；回合开始时变的（turn）不出时间线那一行', () => {
  const store = fresh();
  const s = store.sessions.get('S');
  const transient = (at, kind, body) => { const { seq, ...e } = ev(0, at, kind, 5, body, { kind: 'kernel' }); return e; };
  push(store, transient(1, 'model.changed', { ref: 'cheap', endpoint: 'dev', model: 'small', limits: { window: 64000 }, why: 'turn' }));
  assert.deepEqual(s.model, { ref: 'cheap', endpoint: 'dev', model: 'small' });
  assert.equal(s.limits.window, 64000);
  assert.equal(s.changes.length, 0, 'turn 的不出那一行');
  push(store, transient(2, 'model.changed', { ref: 'dev/m', endpoint: 'dev', model: 'm', effort: { level: 'high', from: 'session' }, why: 'turn' }));
  assert.deepEqual(s.model?.effort, { level: 'high', from: 'session' }, '思考强度（8-18）跟着记');
  push(store, transient(2, 'model.changed', { ref: '@spread', why: 'turn' }));
  assert.deepEqual(s.model, { ref: '@spread' }, '轮换的池只有 ref');
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

test('提前压缩直接换上（6-11）：只来带 prepared 的 compaction.done、前面没有 progress，不出进度那一行，落了盘的那一条照样记上前后的用量', () => {
  const store = fresh();
  const s = store.sessions.get('S');
  const transient = (at, kind, body) => { const { seq, ...e } = ev(0, at, kind, 9, body, { kind: 'kernel' }); return e; };
  push(store, transient(3, 'compaction.done', { seen: 8, trigger: 'auto', before: 812345, after: 31020, prepared: true }));
  assert.equal(s.compacting, null, '不出进度那一行');
  push(store, ev(20, 3, 'context.compacted', 9, { upto: 8, summary: '摘要', trigger: 'auto' }));
  assert.deepEqual(s.compactStats.get(20), { before: 812345, after: 31020 });
  assert.equal(s.compacting, null);
  // 两条谁先到不一定：落了盘的先到
  push(store, ev(30, 4, 'context.compacted', 9, { upto: 25, summary: '摘要', trigger: 'auto' }));
  push(store, transient(5, 'compaction.done', { seen: 25, trigger: 'auto', before: 400000, after: 30000, prepared: true }));
  assert.deepEqual(s.compactStats.get(30), { before: 400000, after: 30000 });
  assert.equal(s.compacting, null);
});

test('提前压好的在线上没写完、失败了换成当场的摘要请求（6-11）：written 变小了当重来，进度那一行从头走', () => {
  const store = fresh();
  const s = store.sessions.get('S');
  const transient = (at, kind, body) => { const { seq, ...e } = ev(0, at, kind, 9, body, { kind: 'kernel' }); return e; };
  push(store, transient(1, 'compaction.progress', { seen: 8, written: 9000, expected: 20000 }));
  const first = s.compacting;
  push(store, transient(2, 'compaction.progress', { seen: 8, written: 0, expected: 21000 }));
  assert.notEqual(s.compacting, first, '重来的是新的一次');
  assert.deepEqual([s.compacting.written, s.compacting.expected, s.compacting.since], [0, 21000, ms(2)]);
  // 后台那次请求出错不带 compaction（只带 purpose）：进度那一行不收
  push(store, ev(20, 3, 'model.called', 9, { seen: 8, result: 'error', purpose: 'compaction', error: { class: 'other', message: 'boom' } }));
  assert.notEqual(s.compacting, null);
});

test('提前压缩直接换上以后清空的那一条不记用量；落了盘以前这一轮先结束了的不留到下一次', () => {
  const store = fresh();
  const s = store.sessions.get('S');
  const transient = (at, kind, body) => { const { seq, ...e } = ev(0, at, kind, 9, body, { kind: 'kernel' }); return e; };
  push(store, transient(3, 'compaction.done', { seen: 8, trigger: 'auto', before: 812345, after: 31020, prepared: true }));
  push(store, ev(21, 3, 'turn.ended', 9, { reason: 'aborted' }));
  push(store, ev(22, 4, 'context.compacted', 9, { upto: 8, trigger: 'clear' }));
  push(store, ev(23, 5, 'context.compacted', 9, { upto: 8, summary: '摘要', trigger: 'manual' }));
  assert.equal(s.compactStats.size, 0);
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
      // 旧核心：没有 `view.page`（核心 9-6 下以前），退回订阅带 `after: 0` 补整份
      if (method === 'view.page') throw Object.assign(new Error('unknown method'), { code: -32601 });
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
  assert.equal(store.summary('S').unread, false, '补的是历史：一轮结束不记成没看过');
  await store.catchUp(s);
  assert.deepEqual(s.events.map((e) => e.seq), [1, 2, 3], '补回来的重复的去掉');
  assert.deepEqual(calls, [['subscribe', 0], ['subscribe', 3]]);
});

/** 一个假核心：会话表照 `listed`，订阅会话的记下来；推送由测试自己推（`push`）。 */
function withIndex(listed) {
  const loaded = [];
  /** @type {((method: string, params: any) => void)[]} */
  const pushes = [];
  const conn = {
    onPush: (fn) => pushes.push(fn),
    request: async (method, params) => {
      if (method === 'subscribe' && params.stream === 'sessions') return { sessions: listed };
      if (method === 'subscribe') loaded.push(params.session);
      return {};
    },
  };
  const store = new Store(/** @type {any} */ (conn));
  return { store, loaded, push: (m, p) => { for (const fn of pushes) fn(m, p); } };
}

const tick = () => new Promise((r) => setTimeout(r, 0));

test('起来时订阅会话表（9-5）：左栏照最近活动列顶层的，一次性的、子会话不列；只读在跑的、派过子代理的，别的不读日志', async () => {
  const listed = [
    { session: 'new', oneshot: false, parent: null, last_active: '2026-10-01T01:00:00.000Z' },
    { session: 'busy', oneshot: false, parent: null, busy: true, last_active: '2026-10-01T05:00:00.000Z' },
    { session: 'mid', oneshot: false, parent: null, last_active: '2026-10-01T03:00:00.000Z', title: '修 CI' },
    { session: 'child', oneshot: false, parent: 'mid', last_active: '2026-10-01T09:00:00.000Z' },
    { session: 'ask', oneshot: true, parent: null, last_active: '2026-10-01T09:30:00.000Z' },
    { session: 'quiet', oneshot: false, parent: null, preview: '帮我看看这个报错', last_active: '2026-10-01T00:30:00.000Z' },
  ];
  const { store, loaded } = withIndex(listed);
  await store.boot();
  await tick();
  assert.deepEqual(store.order, ['busy', 'mid', 'new', 'quiet'], '照最近活动，子代理的、一次性的不列');
  assert.deepEqual(loaded.sort(), ['busy', 'mid'], '在跑的、派过子代理的才读');
  assert.equal(store.summary('quiet').title, '帮我看看这个报错', '没标题的拿第一句话的预览');
  assert.equal(store.summary('mid').title, '修 CI');
  assert.equal(store.summary('busy').running, true);
});

test('会话表推来变化（sessions.changed）：别处开的新会话当场列上；没读的会话一轮结束记成没看过、看了去掉；开始跑的读进来；别处删的交给界面收掉', async () => {
  const { store, loaded, push } = withIndex([{ session: 'a', parent: null, last_active: '2026-10-01T01:00:00.000Z' }]);
  const removed = [];
  store.removed = (id) => removed.push(id);
  await store.boot();
  push('sessions.changed', { session: 'b', entry: { session: 'b', parent: null, last_active: '2026-10-01T02:00:00.000Z' } });
  assert.deepEqual(store.order, ['b', 'a']);
  push('sessions.changed', { session: 'a', entry: { session: 'a', parent: null, busy: true, last_active: '2026-10-01T03:00:00.000Z' } });
  await tick();
  assert.deepEqual(loaded, ['a'], '开始跑的读进来');
  push('sessions.changed', { session: 'b', entry: { session: 'b', parent: null, busy: true, last_active: '2026-10-01T04:00:00.000Z' } });
  store.view('a');
  push('sessions.changed', { session: 'b', entry: { session: 'b', parent: null, last_active: '2026-10-01T05:00:00.000Z' } });
  assert.equal(store.summary('b').unread, true, '没在看的一轮结束了');
  store.view('b');
  assert.equal(store.summary('b').unread, false);
  push('sessions.changed', { session: 'a', removed: true });
  assert.deepEqual(removed, ['a'], '读进来的、不是这一页删的');
  store.leaving('b');
  push('sessions.changed', { session: 'b', removed: true });
  assert.deepEqual(removed, ['a'], '这一页正在删的不再交');
  assert.deepEqual(store.order, []);
});

test('全部会话那一页开的老会话也列进左栏；置顶的排不进最近活动也列', async () => {
  const listed = Array.from({ length: 35 }, (_, i) => ({ session: `s${i}`, parent: null, last_active: new Date(Date.UTC(2026, 9, 1, 0, 60 - i)).toISOString(), ...(i === 34 ? { pinned: true } : {}) }));
  const { store } = withIndex(listed);
  await store.boot();
  assert.equal(store.order.length, 31, '最近的 30 个加上置顶的');
  await store.ensure('s33', true);
  assert.ok(store.order.includes('s33'));
});

test('按页读（核心 9-6 下）：先读最新一页、从它的最后一条往后订阅；往上翻读更早的一页接在前面、照序号去重；订阅回应里的还在跑的任务放成最前面那条种子', async () => {
  const msg = (seq) => ev(seq, seq, 'message.user', undefined, { blocks: [{ type: 'text', text: `第${seq}条` }] });
  const calls = [];
  const conn = {
    onPush() {},
    request: async (method, params) => {
      calls.push([method, params.before ?? params.after ?? null]);
      if (method === 'view.page' && params.before == null) return { events: [5, 6, 7, 8].map(msg), first: 5, last: 8, more: true };
      if (method === 'view.page') return { events: [1, 2, 3, 4, 5].map(msg), first: 1, last: 5, more: false };
      if (method === 'subscribe') return { limits: {}, upto: 8, usage: { usage: { uncached: 10, cache_read: 0, cache_write: 0, output: 5 } }, permission: { level: 'full', read_only: false }, jobs: [{ job: 'j1', what: 'command', title: '编译' }] };
      return {};
    },
  };
  const store = new Store(/** @type {any} */ (conn));
  await store.load('S');
  const s = store.sessions.get('S');
  assert.deepEqual(calls, [['view.page', null], ['subscribe', 8]], '最新一页、从 8 往后订阅');
  assert.deepEqual(s.events.map((e) => `${e.seq}:${e.kind}`), ['0:jobs.seed', '5:message.user', '6:message.user', '7:message.user', '8:message.user']);
  assert.deepEqual([s.first, s.more, s.base?.upto, s.base?.permission?.level], [5, true, 8, 'full']);
  assert.equal(await store.older('S'), true);
  assert.deepEqual(s.events.map((e) => e.seq), [0, 1, 2, 3, 4, 5, 6, 7, 8], '更早的接在前面，5 两页都有、只留一条，种子还在最前面');
  assert.deepEqual([s.first, s.more, s.paged], [1, false, true]);
  assert.equal(await store.older('S'), false, '没有更早的不再读');
});
