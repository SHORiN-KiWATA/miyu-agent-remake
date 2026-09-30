// @ts-check
//! 吉祥物的身子（软件包 `mascot`，蓝图 `web.md`「吉祥物」第 4 条）：走着的时候有人动（按键、动鼠标）就停在原地，不再往前挪
//! （2026-09-30 项目主人指出：走路时一动鼠标，它站着一直往一边滑）。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { Body } from '../../packages/mascot/body.js';

const manifest = JSON.parse(readFileSync(new URL('../../packages/mascot/manifest.json', import.meta.url), 'utf8'));
const config = Object.fromEntries(Object.entries(manifest.settings).map(([k, s]) => [k, s.default]));
Object.assign(globalThis, { innerWidth: 1000, innerHeight: 800 });
const ledge = { id: 'composer', x1: 100, x2: 900, y: 600 };
const floor = { id: 'floor', x1: 0, x2: 1000, y: 800 };
const room = { platforms: () => [ledge, floor], home: () => ({ x: 800, platform: ledge }), size: () => ({ w: 52, h: 48 }), reduced: () => false };

test('走着被叫停：停在原地，之后不再挪', () => {
  const body = new Body(config, room);
  let now = 1000;
  body.move(16, now);
  body.walkTo = 300;
  body.walkAt = now;
  for (let i = 0; i < 20; i++) body.move(16, (now += 16));
  const x = /** @type {any} */ (body.at).x;
  assert.ok(x < 800, '走起来了');
  body.halt();
  for (let i = 0; i < 60; i++) body.move(16, (now += 16));
  assert.equal(/** @type {any} */ (body.at).x, x, '叫停以后不再往前挪');
  assert.equal(/** @type {any} */ (body.at).ground?.id, 'composer', '还站在输入框上');
});

test('跳之前先蹲：蹲的那一会儿脚还在台子上，蹲够了才离地往上', () => {
  const body = new Body(config, room);
  const t0 = performance.now();
  body.move(16, t0);
  body.hop(12);
  body.move(16, t0 + config.jump.windup_ms / 2);
  assert.ok(body.at?.ground, '还在蹲');
  assert.ok(body.crouchAim(t0 + config.jump.windup_ms / 2) > 0, '蹲下去了');
  body.move(16, t0 + config.jump.windup_ms + 20);
  assert.equal(body.at?.ground, null, '起跳了');
  assert.ok(/** @type {any} */ (body.at).vy < 0, '往上');
});

test('按住不拖、松手：按得越久蓄的力越多（最多 1）；拖动过的松手是扔出去', () => {
  const body = new Body(config, room);
  body.move(16, performance.now());
  body.grab(800, 580);
  /** @type {any} */ (body.drag).t0 -= config.jump.charge_ms / 2;
  const half = body.release();
  assert.ok(half != null && half > 0.4 && half < 0.6, `蓄了一半：${half}`);
  body.grab(800, 580);
  /** @type {any} */ (body.drag).t0 -= config.jump.charge_ms * 3;
  assert.equal(body.release(), 1, '蓄满封顶');
  body.grab(800, 580);
  body.dragTo(700, 400);
  assert.equal(body.release(), null, '拖过的是扔');
  assert.equal(body.at?.ground, null, '扔出去在空中');
});

test('出不了窗口：拖到窗口下面、右边外头松手，还是落回窗口里的地上', () => {
  const body = new Body(config, room);
  let now = performance.now();
  body.move(16, now);
  body.grab(800, 580);
  body.dragTo(1200, 1100);
  body.dragTo(1300, 1400);
  body.release();
  for (let i = 0; i < 200; i++) body.move(16, (now += 16));
  const at = /** @type {any} */ (body.at);
  assert.equal(at.ground?.id, 'floor', '落在地上');
  assert.ok(at.y <= 800 && at.x <= 1000 - 26, `在窗口里：${at.x}, ${at.y}`);
});

test('走不到的地方不去：要去的地方在墙外，走到墙边就停下，不原地踏步', () => {
  const body = new Body(config, room);
  let now = performance.now();
  body.move(16, now);
  body.at = { ...(/** @type {any} */ (body.at)), x: 950, y: 800, ground: floor };
  body.walkTo = 1400;
  body.walkAt = now;
  for (let i = 0; i < 200; i++) body.move(16, (now += 16));
  assert.equal(body.walkTo, null, '停下了');
  assert.equal(/** @type {any} */ (body.at).x, 1000 - 26, '停在墙边');
});

test('地上正对输入框的那一段不站：落在那里的走到近的那一边；回输入框时从旁边斜着跳上去', () => {
  const zone = { x1: 60, x2: 940 };
  const side = { ...room, avoid: () => zone };
  const body = new Body(config, side);
  let now = performance.now();
  body.move(16, now);
  // 被扔下去，落在输入框下面靠左（离左边近）
  body.at = { x: 120, y: 700, vx: 0, vy: 0, ground: null };
  for (let i = 0; i < 60 && !body.at.ground; i++) body.move(16, (now += 16));
  assert.equal(body.at.ground?.id, 'floor');
  assert.equal(body.walkTo, 60, '走到近的那一边（左边）');
  for (let i = 0; i < 400 && body.walkTo != null; i++) body.move(16, (now += 16));
  assert.ok(body.at.x <= 60, `走出了那一段：${body.at.x}`);
  // 在右边的地上待够了：回输入框，从旁边斜着跳上去
  body.at = { x: 960, y: 800, vx: 0, vy: 0, ground: { id: 'floor', x1: 0, x2: 1000, y: 800 } };
  body.floorUntil = 0;
  body.wander();
  let landed = null;
  for (let i = 0; i < 400; i++) {
    body.move(16, (now += 16));
    if (body.at.ground?.id === 'composer') { landed = body.at; break; }
  }
  assert.ok(landed, '跳上了输入框');
  assert.ok(landed.x >= 100 && landed.x <= 900, `落在输入框上沿：${landed?.x}`);
});

test('往外让不被叫停；站在那一段里不走的（刷新、改窗口）也会自己让出去', () => {
  const zone = { x1: 60, x2: 940 };
  const side = { ...room, avoid: () => zone };
  const body = new Body(config, side);
  let now = performance.now();
  body.move(16, now);
  // 站在那一段里、没在走（刚起来、窗口改了）：动一下就往外让
  body.at = { x: 900, y: 800, vx: 0, vy: 0, ground: { id: 'floor', x1: 0, x2: 1000, y: 800 } };
  body.move(16, (now += 16));
  assert.equal(body.walkTo, 940, '往近的那一边（右边）让');
  // 人动了鼠标（叫停）：往外让的不停
  body.halt();
  assert.equal(body.walkTo, 940, '往外让的不被叫停');
  for (let i = 0; i < 200 && body.walkTo != null; i++) body.move(16, (now += 16));
  assert.ok(body.at.x >= 940, `让出去了：${body.at.x}`);
});

test('落在那一段正中间、离两边都远：不走过去（一路压着字），就地跳回头顶的输入框', () => {
  const zone = { x1: 60, x2: 940 };
  const body = new Body(config, { ...room, avoid: () => zone });
  let now = performance.now();
  body.move(16, now);
  body.at = { x: 500, y: 700, vx: 0, vy: 0, ground: null };
  let top = null;
  for (let i = 0; i < 400; i++) {
    body.move(16, (now += 16));
    if (body.at.ground?.id === 'composer') { top = body.at; break; }
  }
  assert.ok(top, '跳回了输入框');
  assert.ok(Math.abs(top.x - 500) < 5, `就地跳上去：${top?.x}`);
});

test('窗口窄、两边都站不下（那一段占满了）：不在地上压着字等，就地跳回输入框（2026-10-01）', () => {
  const zone = { x1: 10, x2: 990 };
  const body = new Body(config, { ...room, avoid: () => zone });
  let now = performance.now();
  body.move(16, now);
  body.at = { x: 950, y: 700, vx: 0, vy: 0, ground: null };
  let top = null;
  for (let i = 0; i < 400; i++) {
    body.move(16, (now += 16));
    if (body.at.ground?.id === 'composer') { top = body.at; break; }
  }
  assert.ok(top, '跳回了输入框');
});

test('往下掉的时候框从下面长上来越过了脚（打开抽屉）：顶到框上面，不从框里穿过去掉到地上（2026-10-01）', () => {
  const solid = { ...ledge, bottom: 700 };
  const body = new Body(config, { ...room, platforms: () => [solid, floor] });
  let now = performance.now();
  body.move(16, now);
  // 在框上面往下掉，框上沿已经升过了脚：脚落在框里面
  body.at = { x: 500, y: 620, vx: 0, vy: 0.3, ground: null };
  body.move(16, (now += 16));
  assert.equal(body.at.ground?.id, 'composer', '顶到了框上面');
  assert.equal(body.at.y, 600);
  // 往上跳着的（斜着跳上去）不算：照常飞
  body.at = { x: 500, y: 650, vx: 0, vy: -1, ground: null };
  body.move(16, (now += 16));
  assert.equal(body.at.ground, null, '往上跳的不被顶');
});
