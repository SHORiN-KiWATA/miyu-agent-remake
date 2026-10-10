// @ts-check
//! 一个会话手里的条目（蓝图 `web.md`「照条目画」第 1 条）：换上一页、照推送插、换、挪、接字、藏起、拿掉、换状态，往前翻拼在前面去重。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { ViewLog } from '../src/core/view.js';

const ids = (log) => log.list.map((e) => e.id);

test('订阅回应换上：条目、状态、还有没有更早的', () => {
  const log = new ViewLog();
  log.reset({ entries: [{ id: 'm1', kind: 'user', text: 'hi' }], status: { state: 'idle' }, more: true, first: 7 });
  assert.deepEqual(ids(log), ['m1']);
  assert.deepEqual(log.status, { state: 'idle' });
  assert.equal(log.more, true);
  assert.equal(log.first, 7);
});

test('view.add 插在 after 后面，null 最前，找不到的接在最后；同一条再来换掉', () => {
  const log = new ViewLog();
  log.reset({ entries: [{ id: 'a' }, { id: 'c' }] });
  log.apply('view.add', { entry: { id: 'b' }, after: 'a' });
  log.apply('view.add', { entry: { id: 'z' }, after: null });
  log.apply('view.add', { entry: { id: 'q' }, after: 'nope' });
  assert.deepEqual(ids(log), ['z', 'a', 'b', 'c', 'q']);
  log.apply('view.add', { entry: { id: 'b', v: 2 }, after: 'c' });
  assert.deepEqual(ids(log), ['z', 'a', 'c', 'b', 'q']);
  assert.equal(log.list[3].v, 2);
});

test('view.update 换掉原地；带 after 的挪过去；没有的照 add', () => {
  const log = new ViewLog();
  log.reset({ entries: [{ id: 'a' }, { id: 'b', state: 'running' }, { id: 'c' }] });
  log.apply('view.update', { entry: { id: 'b', state: 'ok' } });
  assert.deepEqual(log.list.map((e) => [e.id, e.state]), [['a', undefined], ['b', 'ok'], ['c', undefined]]);
  log.apply('view.update', { entry: { id: 'a', queued: false }, after: 'c' });
  assert.deepEqual(ids(log), ['b', 'c', 'a']);
  log.apply('view.update', { entry: { id: 'n' }, after: 'b' });
  assert.deepEqual(ids(log), ['b', 'n', 'c', 'a']);
});

test('view.append：回复、思考接 text，工具接 args；换成新对象', () => {
  const log = new ViewLog();
  log.reset({ entries: [{ id: 'r', kind: 'reply', text: 'Hel' }, { id: 't', kind: 'tool', args: '{"a"' }] });
  const before = log.list[0];
  log.apply('view.append', { id: 'r', text: 'lo' });
  log.apply('view.append', { id: 't', text: ':1}' });
  assert.equal(log.list[0].text, 'Hello');
  assert.equal(log.list[1].args, '{"a":1}');
  assert.notEqual(log.list[0], before);
  assert.equal(log.apply('view.append', { id: 'gone', text: 'x' }), false);
});

test('view.hidden 藏起、显示回来；view.remove 拿掉；view.status 整份换', () => {
  const log = new ViewLog();
  log.reset({ entries: [{ id: 'a' }, { id: 'b' }] });
  log.apply('view.hidden', { ids: ['a', 'b'], hidden: true });
  assert.deepEqual(log.list.map((e) => e.hidden), [true, true]);
  log.apply('view.hidden', { ids: ['a'], hidden: false });
  assert.equal('hidden' in log.list[0], false);
  assert.equal(log.apply('view.hidden', { ids: ['a'], hidden: false }), false, '没变的交回没改');
  log.apply('view.remove', { id: 'b' });
  assert.deepEqual(ids(log), ['a']);
  log.apply('view.status', { status: { state: 'running', since: 'x' } });
  assert.equal(log.status.state, 'running');
});

test('往前翻：拼在前面，照 id 去重，记下新的 first', () => {
  const log = new ViewLog();
  log.reset({ entries: [{ id: 'm5' }, { id: 'b6.0' }], more: true, first: 5 });
  log.prepend({ entries: [{ id: 'm1' }, { id: 'm5' }], more: false, first: 1 });
  assert.deepEqual(ids(log), ['m1', 'm5', 'b6.0']);
  assert.equal(log.more, false);
  assert.equal(log.first, 1);
});
