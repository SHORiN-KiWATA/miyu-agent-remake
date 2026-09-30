// @ts-check
//! 吉祥物肚子上的灯怎么闪（软件包 `mascot`，蓝图 `web.md`「吉祥物」第 7 条）：她在回答时一闪一闪；有后台任务在跑、她没在回答时
//! 隔一阵亮一下；别的时候灭着；减少动画的不闪，回答、有后台任务时一直亮。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { lit } from '../../packages/mascot/lamp.js';

const cfg = { busy_on_ms: 300, busy_off_ms: 300, jobs_every_ms: 1800, jobs_on_ms: 250, flash_ms: 120, flashes: 2 };

test('在回答：亮 300 灭 300 轮着来', () => {
  assert.deepEqual([0, 100, 299, 300, 599, 600].map((t) => lit('busy', t, cfg, false)), [true, true, true, false, false, true]);
});

test('有后台任务：隔 1.8 秒亮 0.25 秒；灭着的不亮', () => {
  assert.deepEqual([0, 249, 250, 1799, 1800].map((t) => lit('jobs', t, cfg, false)), [true, true, false, false, true]);
  assert.equal(lit('off', 0, cfg, false), false);
});

test('减少动画：不闪，回答、有后台任务时一直亮', () => {
  assert.equal(lit('busy', 400, cfg, true), true);
  assert.equal(lit('jobs', 900, cfg, true), true);
  assert.equal(lit('off', 0, cfg, true), false);
});
