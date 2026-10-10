// @ts-check
//! 供应商的图标（`src/lib/logo.js`，蓝图 `web.md`「供应商的图标」）：核心给不给、画成哪一种。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { hasLogos, logoPlan, svgUrl } from '../src/lib/logo.js';

const SVG = '<svg viewBox="0 0 24 24"><path fill="currentColor" d="M0 0h24v24H0z"/></svg>';

test('核心给不给图标：有一项带 logo 这一格（哪怕是 null）就是给；老核心没有这一格', () => {
  assert.equal(hasLogos([{ id: 'a' }, { id: 'b', logo: null }]), true);
  assert.equal(hasLogos([{ id: 'a' }]), false);
  assert.equal(hasLogos(null), false);
});

test('画成哪一种：单色照字色、彩色照原样，没有图的写名称的第一个字（大写）', () => {
  assert.deepEqual(logoPlan({ svg: SVG, tint: true }, 'DeepSeek'), { kind: 'tint', url: svgUrl(SVG) });
  assert.equal(logoPlan({ svg: SVG, tint: false }, 'DeepSeek').kind, 'color');
  assert.deepEqual(logoPlan(null, 'openrouter'), { kind: 'initial', text: 'O' });
  assert.deepEqual(logoPlan({ svg: '', tint: true }, '  智谱 GLM'), { kind: 'initial', text: '智' });
  assert.deepEqual(logoPlan(undefined, ''), { kind: 'initial', text: '' });
});

test('data 地址里的引号、尖括号都转义（放进 url("…") 不会断）', () => {
  const url = svgUrl(SVG);
  assert.ok(url.startsWith('data:image/svg+xml;charset=utf-8,'));
  assert.ok(!/["<>]/.test(url));
});
