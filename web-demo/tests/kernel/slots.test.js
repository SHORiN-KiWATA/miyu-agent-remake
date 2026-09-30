// @ts-check
//! 挂载位（蓝图 `web/architecture.md`「挂载位」）：三种的排法；独占的盖住、拿掉露出原来的；按键分派的兜底；没声明的挂进去
//! 报错；声明的包撤回，里面挂的一起收掉；挂的东西画的时候抛错只坏它那一格。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Slots } from '../../src/kernel/slots.js';
import { Registry, Fiber } from '../../src/kernel/context.js';

const entry = (id, extra = {}) => ({ id, render: () => id, ...extra });

test('一串：照 order 排，一样的照 id；同一个 id 后来的盖住先来的，拿掉露出原来的', () => {
  const s = new Slots();
  s.declare('composer.above', 'list', 'dock');
  s.register('composer.above', entry('pulse', { order: 20 }), 'a');
  s.register('composer.above', entry('todo', { order: 10 }), 'b');
  const again = s.register('composer.above', { id: 'todo', order: 5, render: () => 'todo2' }, 'c');
  assert.deepEqual(s.list('composer.above').map((e) => e.render()), ['todo2', 'pulse']);
  again();
  assert.deepEqual(s.list('composer.above').map((e) => e.render()), ['todo', 'pulse']);
});

test('独占：新挂的盖住原来的，拿掉了原来的露出来', () => {
  const s = new Slots();
  s.declare('page.sidebar', 'single', 'shell');
  s.register('page.sidebar', entry('old'), 'a');
  const undo = s.register('page.sidebar', entry('new'), 'b');
  assert.equal(s.single('page.sidebar')?.id, 'new');
  undo();
  assert.equal(s.single('page.sidebar')?.id, 'old');
});

test('按键分派：照键找，找不到用兜底（键是 *）；都没有是 null', () => {
  const s = new Slots();
  s.declare('markdown.code', 'keyed', 'markdown');
  assert.equal(s.pick('markdown.code', 'mermaid'), null);
  s.register('markdown.code', entry('plain', { key: '*' }), 'markdown');
  const undo = s.register('markdown.code', entry('mermaid', { key: 'mermaid' }), 'mermaid');
  assert.equal(s.pick('markdown.code', 'mermaid')?.id, 'mermaid');
  assert.equal(s.pick('markdown.code', 'js')?.id, 'plain');
  undo();
  assert.equal(s.pick('markdown.code', 'mermaid')?.id, 'plain', '拿掉 mermaid 包：兜底接着画');
});

test('没声明的挂进去、同一个名字声明两次、不认识的种类：报错', () => {
  const s = new Slots();
  assert.throws(() => s.register('nowhere', entry('x'), 'a'), /nowhere/);
  s.declare('stage.right', 'list', 'shell');
  assert.throws(() => s.declare('stage.right', 'list', 'other'), /stage\.right/);
  assert.throws(() => s.declare('bad', 'grid', 'shell'), /grid/);
});

test('声明的撤回：里面挂的一起收掉；看着它的收到通知', () => {
  const s = new Slots();
  const seen = [];
  const undeclare = s.declare('stage.right', 'list', 'shell');
  s.watch('stage.right', () => seen.push(s.list('stage.right').length));
  s.register('stage.right', entry('rail'), 'rail');
  undeclare();
  assert.deepEqual(seen, [1, 0]);
  assert.deepEqual(s.list('stage.right'), [], '读没声明的是空的');
  assert.throws(() => s.register('stage.right', entry('rail'), 'rail'), /stage\.right/, '往里挂报错');
});

test('挂的东西画的时候抛错：只坏它那一格，写明是哪个包', () => {
  const s = new Slots();
  s.declare('composer.above', 'list', 'dock');
  s.register('composer.above', { id: 'bad', render: () => { throw new Error('画不出'); } }, 'broken-pkg');
  const out = Slots.draw(s.list('composer.above')[0]);
  assert.deepEqual(out, { failed: true, owner: 'broken-pkg', reason: '画不出' });
});

test('当服务用：经包的上下文挂的东西，跟着那个包撤回', () => {
  const reg = new Registry();
  const slots = new Slots();
  new Fiber(reg, { manifest: { id: 'kernel' }, apply: (ctx) => { ctx.provide('slots', slots); } }, {}).start();
  new Fiber(reg, { manifest: { id: 'dock', inject: ['slots'] }, apply: (ctx) => { ctx.slots.declare('composer.above', 'list'); } }, {}).start();
  const todo = new Fiber(reg, { manifest: { id: 'todo', inject: ['slots'] }, apply: (ctx) => { ctx.slots.register('composer.above', entry('todo')); } }, {});
  todo.start();
  assert.deepEqual(slots.list('composer.above').map((e) => [e.id, e.owner]), [['todo', 'todo']]);
  todo.dispose();
  assert.deepEqual(slots.list('composer.above'), []);
});

test('mount：挂载位还没声明就等着，声明了挂上；撤回了跟着没，再声明又挂上（不看加载的先后）', () => {
  const s = new Slots();
  const undo = s.mount('stage.right', entry('rail'), 'rail');
  assert.deepEqual(s.list('stage.right'), []);
  const undeclare = s.declare('stage.right', 'list', 'shell');
  assert.deepEqual(s.list('stage.right').map((e) => e.id), ['rail']);
  undeclare();
  s.declare('stage.right', 'list', 'shell2');
  assert.deepEqual(s.list('stage.right').map((e) => e.id), ['rail'], '只挂一份');
  undo();
  assert.deepEqual(s.list('stage.right'), []);
});
