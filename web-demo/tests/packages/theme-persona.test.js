// @ts-check
//! 跟着人格的主题色（软件包 `theme-persona`，蓝图「跟着人格的外观」第 1 条）：从头像的像素取主色、照 MD3 生成要盖的那几格。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { seedOf, overlayOf, hexOf } from '../../packages/theme-persona/model.js';

/** 一张 `n` 个像素的图：一种颜色占 `share`，剩下的是另一种（`alpha` 是第二种的不透明度） */
function image(n, main, share, other, alpha = 255) {
  const rgba = [];
  for (let i = 0; i < n; i++) rgba.push(...(i < n * share ? [...main, 255] : [...other, alpha]));
  return rgba;
}

test('主色：照颜色的分布挑最像主题色的那一种；太透明的像素不算；一格都不剩的是 null', () => {
  const blue = seedOf(image(400, [51, 104, 192], 0.7, [250, 242, 233]));
  assert.ok(blue != null);
  const hex = hexOf(/** @type {number} */ (blue));
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
  assert.ok(b > r && b > g, `蓝色占多数的图取出蓝色，实际 ${hex}`);
  // 抠了图的背景（全透明）不算：剩下的只有红
  const red = hexOf(/** @type {number} */ (seedOf(image(400, [192, 40, 60], 0.3, [30, 200, 30], 0))));
  assert.ok(parseInt(red.slice(1, 3), 16) > parseInt(red.slice(3, 5), 16), `透明的绿不算，实际 ${red}`);
  assert.equal(seedOf(image(10, [0, 0, 0], 0, [9, 9, 9], 0)), null);
});

test('照主色生成：浅色、深色各一份，只盖中性色和强调色（晨光的眼睛蓝 #3368c0 对出 MD3 的值）', () => {
  const light = overlayOf('#3368c0', 'light');
  assert.deepEqual(Object.keys(light.page).sort(), ['accent', 'accent_soft', 'gold', 'line', 'line_strong', 'on_primary', 'on_primary_container', 'ribbon', 'sidebar_active', 'sidebar_bg', 'surface', 'surface_1', 'surface_2', 'surface_3', 'text', 'text_soft']);
  assert.deepEqual(Object.keys(light.tui).sort(), ['accent', 'workspace']);
  assert.equal(light.page.surface, '#f9f9ff');
  assert.equal(light.page.surface_1, '#ffffff', '浅色里浮起来的比底亮');
  assert.equal(light.page.accent, '#445e91');
  assert.equal(light.page.text, '#1a1b20');
  const dark = overlayOf('#3368c0', 'dark');
  assert.equal(dark.page.surface, '#111318');
  assert.equal(dark.page.sidebar_bg, '#0c0e13', '深色里左栏比底暗一档');
  assert.equal(dark.page.surface_1, '#1a1b20', '深色里浮起来的也比底亮一档');
  assert.equal(dark.page.accent, '#adc6ff');
  assert.equal(dark.tui.workspace, dark.page.accent);
  assert.equal(light.page.danger, undefined, '出错的红不盖');
});
