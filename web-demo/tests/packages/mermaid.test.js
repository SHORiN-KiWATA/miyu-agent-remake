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

// ---- 核心画（W-4，`mermaid.render`）：三种记号色换成页面的 CSS 变量；画不出的一律交回 null，照代码块写 ----

import { paint, drawer } from '../../packages/mermaid/draw.js';
import { readFileSync } from 'node:fs';

const MARKS = { label: '#070809', line: '#040506', text: '#010203' };
const COLORS = JSON.parse(readFileSync(new URL('../../packages/mermaid/manifest.json', import.meta.url), 'utf8')).settings.colors.default;

test('记号色换成清单里的 CSS 变量：属性、style、<style> 里的都换，大小写不分；别的颜色不动', () => {
  const svg = '<svg><style>.a{fill:#010203}</style><text fill="#010203">A</text><path stroke="#040506" fill="none"/>'
    + '<rect style="fill:#070809" /><rect fill="#ABCDEF"/><text fill="#0A0B0C"/></svg>';
  const out = paint(svg, { ...MARKS, text: '#010203' }, COLORS);
  assert.equal(out, `<svg><style>.a{fill:${COLORS.text}}</style><text fill="${COLORS.text}">A</text><path stroke="${COLORS.line}" fill="none"/>`
    + `<rect style="fill:${COLORS.label}" /><rect fill="#ABCDEF"/><text fill="#0A0B0C"/></svg>`);
  assert.equal(paint('<svg fill="#0A0B0C"/>', { text: '#0a0b0c' }, COLORS), `<svg fill="${COLORS.text}"/>`);
  assert.deepEqual(COLORS, { text: 'var(--text)', line: 'var(--text-soft)', label: 'var(--surface)' });
});

test('问核心画：同一份只问一次；回的 SVG 换好颜色交回', async () => {
  const asked = [];
  const draw = drawer(async (method, params) => {
    asked.push([method, params]);
    return { marks: MARKS, svg: '<svg fill="#010203"/>' };
  }, () => COLORS, () => {});
  assert.equal(await draw('graph TD; A-->B'), '<svg fill="var(--text)"/>');
  assert.equal(await draw('graph TD; A-->B'), '<svg fill="var(--text)"/>');
  assert.deepEqual(asked, [['mermaid.render', { source: 'graph TD; A-->B' }]]);
});

test('核心没带 mermaid、源码空的太长的、画不出、字体读不到：交回 null（照代码块写），原因记一笔', async () => {
  for (const reason of ['unknown_method', 'bad_params', 'mermaid_too_long', 'mermaid_failed', 'internal_error']) {
    const logged = [];
    const draw = drawer(async () => { throw Object.assign(new Error(reason), { reason }); }, () => COLORS, (m) => logged.push(m));
    assert.equal(await draw('x'), null, reason);
    assert.equal(logged.length, 1);
  }
  const odd = drawer(async () => ({ svg: 42 }), () => COLORS, () => {});
  assert.equal(await odd('x'), null, '回应不像样的也当画不出');
});
