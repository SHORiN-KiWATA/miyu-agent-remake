// @ts-check
//! 上下文（蓝图 `web/architecture.md`「上下文（系统调用）」）：撤回照反序、只撤一次；缺服务等着、来了就绪、换了重来；
//! 没声明的服务碰了报错；`apply` 抛错只它故障。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Registry, Fiber, useLanguage } from '../../src/kernel/context.js';

/** 一个包：清单加 `apply`。 */
const pkg = (id, inject, apply) => ({ manifest: { id, inject, settings: {} }, apply });

test('撤回照登记的反序，每件只撤一次', () => {
  const log = [];
  const reg = new Registry();
  const fiber = new Fiber(reg, pkg('a', [], (ctx) => {
    ctx.effect(() => { log.push('做 1'); return () => log.push('撤 1'); });
    const undo = ctx.effect(() => { log.push('做 2'); return () => log.push('撤 2'); });
    undo();
    undo();
    ctx.effect(() => { log.push('做 3'); return () => log.push('撤 3'); });
  }), {});
  fiber.start();
  fiber.dispose();
  fiber.dispose();
  assert.deepEqual(log, ['做 1', '做 2', '撤 2', '做 3', '撤 3', '撤 1']);
  assert.equal(fiber.state, 'disposed');
});

test('要的服务不在：等着，写明缺哪个；来了就绪；拿掉了又等着，它做过的撤回', () => {
  const reg = new Registry();
  const log = [];
  const user = new Fiber(reg, pkg('user', ['theme'], (ctx) => {
    log.push(`用 ${ctx.theme.name}`);
    ctx.effect(() => () => log.push('撤回'));
  }), {});
  user.start();
  assert.equal(user.state, 'pending');
  assert.deepEqual(user.missing(), ['theme']);
  const provider = new Fiber(reg, pkg('theme-a', [], (ctx) => { ctx.provide('theme', { name: '晨光' }); }), {});
  provider.start();
  assert.equal(user.state, 'active');
  provider.dispose();
  assert.equal(user.state, 'pending');
  assert.deepEqual(log, ['用 晨光', '撤回']);
});

test('要的服务换了提供者：整个包撤回、照新的重来一遍', () => {
  const reg = new Registry();
  const log = [];
  new Fiber(reg, pkg('user', ['theme'], (ctx) => {
    const name = ctx.theme.name;
    log.push(`用 ${name}`);
    ctx.effect(() => () => log.push(`撤 ${name}`));
  }), {}).start();
  const a = new Fiber(reg, pkg('a', [], (ctx) => { ctx.provide('theme', { name: '晨光' }); }), {});
  a.start();
  a.dispose();
  new Fiber(reg, pkg('b', [], (ctx) => { ctx.provide('theme', { name: 'tokyonight' }); }), {}).start();
  assert.deepEqual(log, ['用 晨光', '撤 晨光', '用 tokyonight']);
});

test('带 ? 的服务有就用、没有也就绪；后来有了照它重来', () => {
  const reg = new Registry();
  const seen = [];
  const user = new Fiber(reg, pkg('user', ['lightbox?'], (ctx) => { seen.push(ctx.lightbox ? '有灯箱' : '没灯箱'); }), {});
  user.start();
  assert.equal(user.state, 'active');
  new Fiber(reg, pkg('lb', [], (ctx) => { ctx.provide('lightbox', {}); }), {}).start();
  assert.deepEqual(seen, ['没灯箱', '有灯箱']);
});

test('没在 inject 里写的服务碰了报错：算这个包故障，写明原因', () => {
  const reg = new Registry();
  new Fiber(reg, pkg('p', [], (ctx) => { ctx.provide('secret', 1); }), {}).start();
  const nosy = new Fiber(reg, pkg('nosy', [], (ctx) => { void ctx.secret; }), {});
  nosy.start();
  assert.equal(nosy.state, 'failed');
  assert.match(nosy.reason ?? '', /secret/);
});

test('apply 抛错：只它故障，做到一半的撤回；别的包照常', () => {
  const reg = new Registry();
  const log = [];
  const bad = new Fiber(reg, pkg('bad', [], (ctx) => {
    ctx.effect(() => () => log.push('撤回半截'));
    throw new Error('坏了');
  }), {});
  bad.start();
  const good = new Fiber(reg, pkg('good', [], () => {}), {});
  good.start();
  assert.equal(bad.state, 'failed');
  assert.equal(bad.reason, '坏了');
  assert.equal(good.state, 'active');
  assert.deepEqual(log, ['撤回半截']);
});

test('一件撤回抛错不耽误别的撤回', () => {
  const reg = new Registry();
  const log = [];
  const f = new Fiber(reg, pkg('p', [], (ctx) => {
    ctx.effect(() => () => log.push('撤 1'));
    ctx.effect(() => () => { throw new Error('撤不掉'); });
  }), {});
  f.start();
  f.dispose();
  assert.deepEqual(log, ['撤 1']);
});

test('事件：听的包撤回了就不再听', () => {
  const reg = new Registry();
  const heard = [];
  const ear = new Fiber(reg, pkg('ear', [], (ctx) => { ctx.on('turn.ended', (n) => heard.push(n)); }), {});
  ear.start();
  reg.emit('turn.ended', 1);
  ear.dispose();
  reg.emit('turn.ended', 2);
  assert.deepEqual(heard, [1]);
});

test('状态事件（publish）：记着最后一份，后来才听的先拿到它（刷新时包比页面晚起来，运行状态行等下一件事才出来）；普通事件不补', () => {
  const reg = new Registry();
  reg.publish('view.changed', { running: true });
  reg.emit('turn.ended', 1);
  const got = [];
  reg.on('view.changed', (v) => got.push(v));
  reg.on('turn.ended', (n) => got.push(n));
  assert.deepEqual(got, [{ running: true }], '状态补一份，turn.ended 不补');
  reg.publish('view.changed', { running: false });
  assert.deepEqual(got, [{ running: true }, { running: false }]);
});

test('服务可以按调它的包绑一份：经它登记的东西跟着这个包撤回', () => {
  const reg = new Registry();
  const table = new Set();
  // 像挂载位：按包绑一份，登记走那个包的 effect
  const service = { bind: (ctx) => ({ add: (x) => ctx.effect(() => { table.add(x); return () => table.delete(x); }) }) };
  new Fiber(reg, pkg('kernel', [], (ctx) => { ctx.provide('table', service); }), {}).start();
  const user = new Fiber(reg, pkg('user', ['table'], (ctx) => { ctx.table.add('x'); }), {});
  user.start();
  assert.deepEqual([...table], ['x']);
  user.dispose();
  assert.deepEqual([...table], []);
});

test('配置和字：包拿到自己的设置项最终值和自己的字', () => {
  const reg = new Registry();
  let got = null;
  const f = new Fiber(reg, {
    manifest: { id: 'p', inject: [], settings: {}, text: { zh: { hi: '你好 {name}' } } },
    apply: (ctx) => { got = [ctx.config.rows, ctx.text('hi', { name: 'Miyu' })]; },
  }, { rows: 5 });
  f.start();
  assert.deepEqual(got, [5, '你好 Miyu']);
});

test('带 ~ 的服务：用的时候再找（总是现在的那个），来了、换了、没了都不让这个包重来', () => {
  const reg = new Registry();
  let starts = 0;
  let ctxSeen = null;
  const user = new Fiber(reg, pkg('user', ['lightbox~'], (ctx) => { starts += 1; ctxSeen = ctx; }), {});
  user.start();
  assert.equal(user.state, 'active');
  assert.equal(ctxSeen.lightbox, undefined);
  const lb = new Fiber(reg, pkg('lb', [], (ctx) => { ctx.provide('lightbox', { name: 'a' }); }), {});
  lb.start();
  assert.equal(ctxSeen.lightbox?.name, 'a', '用的时候找得到新来的');
  lb.dispose();
  assert.equal(ctxSeen.lightbox, undefined);
  assert.equal(starts, 1, '一直没重来');
});

test('字照界面语言取：这一种没有的那一句退回表里的那一种；ctx.local 挑按语言写的一块', () => {
  useLanguage({ code: 'ja', fallback: 'zh' });
  try {
    const reg = new Registry();
    let got = null;
    const f = new Fiber(reg, {
      manifest: { id: 'p', inject: [], settings: {}, text: { zh: { hi: '你好', bye: '再见' }, ja: { hi: 'こんにちは' } } },
      apply: (ctx) => { got = [ctx.text('hi'), ctx.text('bye'), ctx.local({ zh: ['想'], ja: ['考え'] }), ctx.local(['原样'])]; },
    }, {});
    f.start();
    assert.deepEqual(got, ['こんにちは', '再见', ['考え'], ['原样']]);
  } finally {
    useLanguage({ code: 'zh', fallback: 'zh' });
  }
});
