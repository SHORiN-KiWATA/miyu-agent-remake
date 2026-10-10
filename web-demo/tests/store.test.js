// @ts-check
//! 会话仓库（`core/store.js`）：会话表、读进来的会话照视图流（核心 9-8）换上、照推送改、往上翻、掉队重来；左栏的一项、没看过的。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadRes } from './support.js';
import { Store, emptySession } from '../src/core/store.js';

loadRes();

const AT = '2026-10-10T04:02:00.000Z';

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

test('读一个会话：订阅视图流，换上最新的一页和会话状态；往上翻读更早的一页接在前面、照编号去重；没有更早的不再读', async () => {
  const calls = [];
  const conn = {
    onPush() {},
    request: async (method, params) => {
      calls.push([method, params.stream ?? null, params.before ?? null]);
      if (method === 'subscribe') return {
        entries: [{ id: 'm5', kind: 'user', text: '第5条', turn: 5, at: AT }, { id: 'e6', kind: 'end', turn: 5, at: AT }], first: 5, last: 6, more: true,
        status: { state: 'idle', model: { endpoint: 'dev', model: 'm1' }, todos: [{ content: '写测试', status: 'pending' }] },
        limits: { window: 1000 }, usage: { usage: { uncached: 10, cache_read: 0, cache_write: 0, output: 5 } }, permission: { level: 'full', read_only: false }, workspace: { cwd: '/w' },
      };
      if (method === 'view.page') return { entries: [{ id: 'm1', kind: 'user', text: '第1条', turn: 1, at: AT }, { id: 'm5', kind: 'user', text: '第5条', turn: 5, at: AT }], first: 1, more: false };
      return {};
    },
  };
  const store = new Store(/** @type {any} */ (conn));
  await store.load('S');
  const s = /** @type {any} */ (store.sessions.get('S'));
  assert.deepEqual(calls, [['subscribe', 'view', null]]);
  assert.deepEqual(s.view.list.map((e) => e.id), ['m5', 'e6']);
  assert.deepEqual([s.first, s.more, s.limits.window, s.model.model, s.todos.length, s.base.permission.level, s.base.workspace.cwd], [5, true, 1000, 'm1', 1, 'full', '/w']);
  assert.equal(await store.older('S'), true);
  assert.deepEqual(calls.at(-1), ['view.page', null, 5]);
  assert.deepEqual(s.view.list.map((e) => e.id), ['m1', 'm5', 'e6'], 'm5 两页都有、只留一条');
  assert.deepEqual([s.first, s.more, s.paged], [1, false, true]);
  assert.equal(await store.older('S'), false, '没有更早的不再读');
});

test('视图流的推送照编号改条目；排着的话被退回的告诉界面放回输入框；别的会话、不是视图流的推送不管', () => {
  const store = new Store(/** @type {any} */ ({ onPush() {}, request: async () => ({}) }));
  store.sessions.set('S', emptySession('S'));
  const back = [];
  store.withdrawn = (session, text) => back.push([session, text]);
  let changed = 0;
  store.on(() => { changed += 1; });
  store.push('view.add', { session: 'S', entry: { id: 'm1', kind: 'user', text: '排着的', queued: true }, after: null });
  store.push('view.update', { session: 'S', entry: { id: 'm1', kind: 'user', text: '排着的', queued: true, withdrawn: true } });
  store.push('view.update', { session: 'S', entry: { id: 'm1', kind: 'user', text: '排着的', queued: true, withdrawn: true } });
  assert.deepEqual(back, [['S', '排着的']], '退回只说一次');
  store.push('view.add', { session: 'X', entry: { id: 'm9' }, after: null });
  store.push('event', { session: 'S', event: { seq: 1 } });
  assert.deepEqual(store.sessions.get('S')?.view.list.map((e) => e.id), ['m1']);
  assert.equal(changed, 3);
});

test('会话状态换了：待办、模型跟着换；待办做完清空的留着清空前那一份；一轮做完了、没在看的记成没看过', () => {
  const store = new Store(/** @type {any} */ ({ onPush() {}, request: async () => ({}) }));
  store.sessions.set('S', emptySession('S'));
  const s = /** @type {any} */ (store.sessions.get('S'));
  const status = (more) => store.push('view.status', { session: 'S', status: { state: 'running', ...more } });
  status({ todos: [{ content: 'a', status: 'in_progress' }], model: { endpoint: 'dev', model: 'm2' } });
  assert.deepEqual([s.todos.length, s.todosDone, s.model.model], [1, null, 'm2']);
  status({ todos: [{ content: 'a', status: 'completed' }] });
  status({ todos: [] });
  assert.deepEqual(s.todosDone, [{ content: 'a', status: 'completed' }], '全做完了清空的');
  store.view('other');
  store.push('view.status', { session: 'S', status: { state: 'idle' } });
  assert.equal(store.summary('S').unread, true);
  assert.equal(store.summary('S').running, false);
  store.view('S');
  assert.equal(store.summary('S').unread, false);
});

test('掉了队（resync）：重新订阅视图流、换上最新的一页；正在删的不补', async () => {
  let n = 0;
  const conn = {
    onPush() {},
    request: async (method) => (method === 'subscribe' ? { entries: [{ id: `m${++n}`, kind: 'user', text: 'x' }], status: { state: 'idle' } } : {}),
  };
  const store = new Store(/** @type {any} */ (conn));
  await store.load('S');
  store.push('resync', { session: 'S' });
  await tick();
  assert.deepEqual(store.sessions.get('S')?.view.list.map((e) => e.id), ['m2']);
  store.leaving('S');
  store.push('resync', { session: 'S' });
  await tick();
  assert.equal(n, 2, '正在删的不再订阅');
});

test('读进来了的会话：标题、置顶照会话表；在不在跑照会话状态；会话表里没有的没有标题', async () => {
  const { store, push } = withIndex([{ session: 'L', parent: null, title: '修设置页', pinned: true, last_active: '2026-10-08T01:00:00.000Z' }, { session: 'Q', parent: null, preview: '整个会话的第一句', busy: true, last_active: '2026-10-08T00:00:00.000Z' }]);
  await store.boot();
  for (const id of ['L', 'Q', 'X']) store.sessions.set(id, emptySession(id));
  store.sessions.get('Q')?.view.reset({ entries: [], status: { state: 'idle' } });
  assert.deepEqual([store.summary('L').title, store.summary('L').pinned], ['修设置页', true]);
  assert.equal(store.summary('Q').title, '整个会话的第一句');
  assert.equal(store.summary('Q').running, false, '读进来了的照会话状态，不照会话表的 busy');
  assert.equal(store.summary('X').title, null);
  push('sessions.changed', { session: 'L', entry: { session: 'L', parent: null, title: '改过的名字', last_active: '2026-10-08T02:00:00.000Z' } });
  assert.deepEqual([store.summary('L').title, store.summary('L').pinned], ['改过的名字', false]);
});
