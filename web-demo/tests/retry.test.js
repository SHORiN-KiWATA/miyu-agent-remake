// @ts-check
//! 重试（蓝图 `web.md`「运行状态行」、`kernel/events.md` 瞬时事件第 17 条）：瞬时的 `status` 带着 `retry` 就记下来，
//! 下一段 `model.delta` 来了、这一轮结束了就去掉；样本照 `docs/designs/samples/transient/status.jsonl`。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { loadRes, ev } from './support.js';
import { Store, emptySession } from '../src/core/store.js';

loadRes();

/** 样本一行一条：第一条连不上（没有状态码），第二条被限速（带状态码 429，施工 3-5 三补）。 */
const [SAMPLE, LIMITED] = readFileSync(new URL('../../docs/designs/samples/transient/status.jsonl', import.meta.url), 'utf8')
  .trim().split('\n').map((line) => JSON.parse(line));

function fresh() {
  const store = new Store(/** @type {any} */ ({ onPush() {}, request: async () => ({}) }));
  store.sessions.set('S', emptySession('S'));
  return store;
}

const push = (store, event) => store.push('event', { session: 'S', event });

test('status 带着 retry：记下第几次、一共几次、原话、哪一轮、什么时候重试（事件的时刻加 wait_ms）', () => {
  const store = fresh();
  push(store, SAMPLE);
  assert.deepEqual(store.sessions.get('S')?.retry, { turn: 42, attempt: 1, limit: 5, message: 'connection reset by peer', failover: false,
    due: Date.parse(SAMPLE.at) + SAMPLE.body.retry.wait_ms });
  push(store, LIMITED);
  assert.deepEqual(store.sessions.get('S')?.retry, { turn: 117, attempt: 1, limit: 5, message: 'HTTP 429: Rate limit reached', failover: false,
    due: Date.parse(LIMITED.at) + LIMITED.body.retry.wait_ms });
});

test('下一段 model.delta 来了：重试过去了', () => {
  const store = fresh();
  push(store, SAMPLE);
  const { seq, ...delta } = ev(0, 1, 'model.delta', 42, { seen: 44, index: 0, start: 'text' });
  push(store, delta);
  assert.equal(store.sessions.get('S')?.retry, null);
});

test('这一轮结束了：重试去掉', () => {
  const store = fresh();
  push(store, SAMPLE);
  push(store, ev(90, 2, 'turn.ended', 42, { reason: 'error' }));
  assert.equal(store.sessions.get('S')?.retry, null);
});

test('不带 retry 的 status 不管', () => {
  const store = fresh();
  push(store, { ...SAMPLE, body: { seen: 44 } });
  assert.equal(store.sessions.get('S')?.retry, null);
});
