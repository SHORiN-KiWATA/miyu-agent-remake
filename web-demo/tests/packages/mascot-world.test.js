// @ts-check
//! 吉祥物的重力（软件包 `mascot`，蓝图 `web.md`「吉祥物」第 2–4 条）：越掉越快，落在下面第一个台子上；台子升起来把它顶上去；
//! 走出台子的边就掉；地是最后一个台子。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { step, supportAt } from '../../packages/mascot/world.js';

const floor = { x1: 0, x2: 1000, y: 800 };
const ledge = { x1: 300, x2: 700, y: 600 };
const body = (x, y, extra = {}) => ({ x, y, vx: 0, vy: 0, ground: null, ...extra });

test('掉：越掉越快，落在下面第一个台子上；台子外面的落到地上', () => {
  let b = body(500, 100);
  const speeds = [];
  for (let i = 0; i < 200 && !b.ground; i++) { b = step(b, 16, [ledge, floor], 2400).body; speeds.push(b.vy); }
  assert.ok(speeds[5] > speeds[1], '越掉越快');
  assert.equal(b.y, 600);
  assert.deepEqual(b.ground, ledge);
  let c = body(100, 100);
  for (let i = 0; i < 300 && !c.ground; i++) c = step(c, 16, [ledge, floor], 2400).body;
  assert.equal(c.y, 800);
});

test('落地那一下交回落得多快（压扁多少照它）', () => {
  let b = body(500, 500);
  let hit = null;
  for (let i = 0; i < 100 && !hit; i++) { const r = step(b, 16, [ledge, floor], 2400); b = r.body; hit = r.landed; }
  assert.ok(hit && hit > 0);
});

test('台子升起来：脚在它上面一点的范围里的被顶到台子上；走出台子的边就掉', () => {
  const b = body(500, 600, { ground: ledge });
  const risen = { x1: 300, x2: 700, y: 520 };
  assert.equal(supportAt(b, [risen, floor], 90)?.y, 520, '台子升到脚上面不远：顶上去');
  let w = body(690, 600, { ground: ledge, vx: 200 });
  for (let i = 0; i < 10; i++) w = step(w, 16, [ledge, floor], 2400).body;
  assert.equal(w.ground, null, '走出了台子：往下掉');
});
