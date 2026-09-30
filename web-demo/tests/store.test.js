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
