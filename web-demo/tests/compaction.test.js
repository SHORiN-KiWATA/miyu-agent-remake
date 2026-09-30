// @ts-check
//! 压缩的进度（蓝图 `web.md`「压缩的进度」第 3 条）：条分 24 格，按整格一顿一顿地追真实的字数，不超过真实够的格数，最多 95%。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { realCells, chase, percent } from '../src/model/compaction.js';

test('真实够几格：已写 ÷ 估计，最多到 95%；没给估计的是 0', () => {
  assert.equal(realCells(5000, 20000, 24, 0.95), 6);
  assert.equal(realCells(40000, 20000, 24, 0.95), 22);
  assert.equal(realCells(100, 0, 24, 0.95), 0);
});

test('追一步：多走 1–3 格，不超过真实够的；已经够了不动', () => {
  assert.equal(chase(2, 10, () => 0), 3);
  assert.equal(chase(2, 10, () => 0.99), 5);
  assert.equal(chase(9, 10, () => 0.99), 10);
  assert.equal(chase(10, 10, () => 0.5), 10);
});

test('百分比照真实的字数，最多 95%', () => {
  assert.equal(percent(3120, 20000, 0.95), 16);
  assert.equal(percent(30000, 20000, 0.95), 95);
});
