// @ts-check
//! 照条目和会话状态算的几样（蓝图 `web.md`「照条目画」第 3、4 条）：开到第几轮、这一轮结束没有、在跑的这一轮她做没做事、压缩的进度。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { lastTurn, turnEnded, untouchedTurnOf, compactingOf } from '../src/model/view-state.js';

const AT = '2026-10-10T04:02:00.000Z';
const user = { id: 'm41', kind: 'user', text: '看看 src', turn: 42, at: AT };
const running = { state: 'running', since: AT };

test('开到第几轮照最后一条带回合的；这一轮有了收尾那一条才算结束', () => {
  assert.equal(lastTurn([]), 0);
  assert.equal(lastTurn([{ id: 'm1', kind: 'user', turn: 3 }, { id: 'c2', kind: 'notice', what: 'cleared' }, { id: 'm4', kind: 'user', turn: 5 }]), 5);
  assert.equal(turnEnded([user], 42), false);
  assert.equal(turnEnded([user, { id: 'e9', kind: 'end', turn: 42, reason: 'completed' }], 42), true);
});

test('打断时她还没开始做事：只在思考、在等模型的算没开始；写出了正文、调了工具算开始；没在跑的、结束了的不算', () => {
  assert.equal(untouchedTurnOf([user], running), 42, '还在等模型');
  assert.equal(untouchedTurnOf([user, { id: 'b1', kind: 'thought', turn: 42, text: '想一想', open: true }], running), 42, '只在思考');
  assert.equal(untouchedTurnOf([user, { id: 'b1', kind: 'reply', turn: 42, text: '', open: true }], running), 42, '正文开了还没字');
  assert.equal(untouchedTurnOf([user, { id: 'b1', kind: 'reply', turn: 42, text: '我先', open: true }], running), null, '写出了正文');
  assert.equal(untouchedTurnOf([user, { id: 'b1', kind: 'tool', turn: 42, name: 'read', state: 'preparing' }], running), null, '在调工具');
  assert.equal(untouchedTurnOf([user, { id: 'e9', kind: 'end', turn: 42 }], running), null, '这一轮已经结束了');
  assert.equal(untouchedTurnOf([user], { state: 'idle' }), null, '没在跑');
  assert.equal(untouchedTurnOf([], null), null);
});

test('压缩的进度：会话状态在压的照它写了多少、估计多少，从那一条的时刻起；不在压的是 null', () => {
  const entries = [{ id: 'c20', kind: 'notice', what: 'compaction', state: 'running', at: AT }];
  const got = compactingOf(entries, { state: 'running', doing: { what: 'compacting', entry: 'c20', written: 1200, expected: 4000 } });
  assert.deepEqual(got, { seen: 20, since: Date.parse(AT), written: 1200, expected: 4000, trigger: 'manual' });
  const auto = [{ ...entries[0], trigger: 'auto' }];
  assert.equal(compactingOf(auto, { state: 'running', doing: { what: 'compacting', entry: 'c20' } })?.trigger, 'auto', '照那一条旁白');
  assert.equal(compactingOf(entries, { state: 'running', doing: { what: 'compacting', entry: 'c20', trigger: 'auto' } })?.trigger, 'auto', '会话状态给了的照它');
  assert.equal(compactingOf(entries, { state: 'running', doing: { what: 'replying' } }), null);
  assert.equal(compactingOf(entries, null), null);
});
