// @ts-check
//! 会话表（核心 9-5 的会话列表流）：整张表、整项换、删掉、掉队重订、旧核心退回读一次。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { SessionIndex } from '../src/core/session-index.js';

/** 一个假核心：`subscribe` 交 `lists` 里的下一张表；`sessions` 流不认的（`old`）退回 `session.list`。 */
function fake(lists, old = false) {
  const calls = [];
  /** @type {any} */
  let push = null;
  const conn = {
    onPush: (fn) => { push = fn; },
    request: async (method, params) => {
      calls.push(method === 'subscribe' ? `subscribe:${params.stream}` : method);
      if (method === 'subscribe' && old) throw Object.assign(new Error('bad stream'), { reason: 'bad_params' });
      return { sessions: lists.shift() ?? [] };
    },
  };
  const index = new SessionIndex(/** @type {any} */ (conn));
  const changes = [];
  index.on((id, before, after) => changes.push([id, before?.busy ?? null, after ? (after.busy ?? false) : null]));
  return { index, calls, changes, push: (m, p) => push(m, p) };
}

test('订阅拿到整张表；整项换上，新来的排在最前，删掉的拿掉；每一项变了告诉听的（之前、之后）', async () => {
  const { index, changes, push } = fake([[{ session: 'a' }, { session: 'b', busy: true }]]);
  await index.start();
  assert.equal(index.live, true);
  assert.deepEqual(index.all().map((e) => e.session), ['a', 'b']);
  push('sessions.changed', { session: 'c', entry: { session: 'c' } });
  push('sessions.changed', { session: 'b', entry: { session: 'b' } });
  push('sessions.changed', { session: 'a', removed: true });
  push('sessions.changed', { session: 'zzz', removed: true });
  assert.deepEqual(index.all().map((e) => e.session), ['c', 'b']);
  assert.deepEqual(changes, [['a', null, false], ['b', null, true], ['c', null, false], ['b', true, false], ['a', null, null]]);
});

test('掉了队（resync 的 sessions）重新订阅：整张换上，和手里的比，只报变了的、没了的算删掉', async () => {
  const { index, calls, changes, push } = fake([[{ session: 'a' }, { session: 'b' }], [{ session: 'a' }, { session: 'c', title: 'x' }]]);
  await index.start();
  changes.length = 0;
  push('resync', { stream: 'sessions' });
  push('resync', { session: 'a', stream: 'events' });
  await new Promise((r) => setTimeout(r, 0));
  assert.deepEqual(calls, ['subscribe:sessions', 'subscribe:sessions'], '会话的事件流掉队不归它管');
  assert.deepEqual(changes, [['b', null, null], ['c', null, false]]);
});

test('自己刚开的会话先记上一项，推送到了整项换掉；旧核心没有这个流的退回 session.list 读一次', async () => {
  const { index, push } = fake([[{ session: 'a' }]]);
  await index.start();
  index.seed('n', { cwd: '~' });
  assert.deepEqual(index.all().map((e) => e.session), ['n', 'a']);
  push('sessions.changed', { session: 'n', entry: { session: 'n', title: '新' } });
  assert.equal(index.get('n')?.title, '新');
  const old = fake([[{ session: 'x' }]], true);
  await old.index.start();
  assert.deepEqual([old.index.live, old.calls], [false, ['subscribe:sessions', 'session.list']]);
  assert.equal(old.index.get('x')?.session, 'x');
});
