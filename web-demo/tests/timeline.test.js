// @ts-check
//! 时间线的一段里转圈的那一步（`tui.md`「时间线」第 19 条）：段和步照核心的条目拼（`model/entries.js`）。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadRes } from './support.js';
import { itemsOf } from '../src/model/entries.js';
import { active } from '../src/model/timeline.js';

loadRes();

const AT = '2026-10-10T04:02:00.000Z';
const segment = (steps) => {
  const entries = [
    { id: 'm1', kind: 'user', text: '跑一下', turn: 1, at: AT },
    { id: 'g2', kind: 'group', turn: 1, open: true, steps: steps.map((s) => s.id), at: AT },
    ...steps.map((s) => ({ group: 'g2', turn: 1, at: AT, ...s })),
  ];
  return itemsOf(entries, { state: 'running', since: AT }).items.find((it) => it.type === 'steps');
};

test('在跑的：参数写完了等结果的，转最前面那个没结果的，后面的排队', () => {
  const seg = segment([
    { id: 'b1', kind: 'thought', text: '想一想', took_ms: 300 },
    { id: 'b2', kind: 'tool', name: 'shell', state: 'running', args: '{"command":"ls"}' },
    { id: 'b3', kind: 'tool', name: 'shell', state: 'running', args: '{"command":"pwd"}' },
  ]);
  assert.equal(seg.finished, false);
  assert.deepEqual(seg.steps.map((s) => s.state), ['done', 'running', 'running']);
  assert.equal(active(seg), 1);
});

test('她还在写的（在想、在写参数）转最后那一步；都有结果了不转', () => {
  const writing = segment([
    { id: 'b1', kind: 'thought', text: '想一想', took_ms: 300 },
    { id: 'b2', kind: 'tool', name: 'shell', state: 'preparing', args: '{"command":"l' },
  ]);
  assert.equal(active(writing), 1);
  const thinking = segment([{ id: 'b1', kind: 'thought', text: '想', open: true }]);
  assert.equal(active(thinking), 0);
  const done = segment([{ id: 'b1', kind: 'tool', name: 'shell', state: 'ok', args: '{}', took_ms: 10 }]);
  assert.equal(active(done), null);
});
