// @ts-check
//! 软件包 mermaid：挂进 `markdown.code` 的键 `mermaid`；停用了（没人接这个键）回答里的 mermaid 照代码块写；画法抛错的也照代码块写。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadRes } from '../support.js';
import { Slots } from '../../src/kernel/slots.js';
import { richHooks } from '../../src/ui/rich.js';

loadRes();

const where = { session: 's', home: '/home/me', cwd: '/home/me' };
const say = () => {};

/** 声明了 `markdown.code` 的挂载位，按包绑的一份（像软件包 app 拿到的 `ctx.slots`）。 */
function slots() {
  const s = new Slots();
  s.declare('markdown.code', 'keyed', 'app');
  return { raw: s, api: s.bind({ id: 'app', effect: (fn) => fn() }) };
}

test('没装 mermaid：mermaid 代码块照代码块写（扩展点交回 null）', () => {
  const { api } = slots();
  assert.equal(richHooks(where, say, { slots: api })('1:0').code?.('mermaid', 'graph TD; A-->B', true), null);
});

test('装了：收齐的交给它画，没收齐的照代码块写；拿掉了又照代码块写', () => {
  const { raw, api } = slots();
  const node = { nodeType: 1, text: '' };
  const undo = raw.register('markdown.code', { id: 'mermaid', key: 'mermaid', render: ({ text }) => ({ ...node, text }) }, 'mermaid');
  const hooks = richHooks(where, say, { slots: api })('1:0');
  assert.equal(hooks.code?.('mermaid', 'graph TD; A-->B', true)?.text, 'graph TD; A-->B');
  assert.equal(hooks.code?.('mermaid', 'graph TD; A-->', false), null);
  undo();
  assert.equal(richHooks(where, say, { slots: api })('1:0').code?.('mermaid', 'graph TD; A-->B', true), null);
});

test('画法抛错：照代码块写，别的照常', () => {
  const { raw, api } = slots();
  raw.register('markdown.code', { id: 'mermaid', key: 'mermaid', render: () => { throw new Error('坏了'); } }, 'mermaid');
  assert.equal(richHooks(where, say, { slots: api })('1:0').code?.('mermaid', 'x', true), null);
});
