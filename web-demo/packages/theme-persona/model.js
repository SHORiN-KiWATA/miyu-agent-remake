// @ts-check
//! 跟着人格的主题色怎么算（蓝图 `web.md`「主题」的「跟着人格的外观」第 1 条）：从头像的像素取主色，照 MD3 `TONAL_SPOT`、对比度 0
//! 生成一套，换成要盖在选中的那一套上面的几格。纯函数；读图、挂上去在 `index.js`。
//!
//! 只盖中性色和强调色：出错的红、差异的红绿、链接、代码高亮、吉祥物的颜色照选中的那一套（语义色跟着人格变了反而认不出）。

import { QuantizerCelebi, Score, Hct, SchemeTonalSpot, MaterialDynamicColors as D, argbFromRgb, argbFromHex, hexFromArgb } from './vendor/mcu.min.js';

/** 取主色时最多挑多少种颜色（MD3 取壁纸色的做法） */
const QUANTIZE = 128;
/** 太透明的像素不算（头像的圆角外面、抠了图的背景） */
const MIN_ALPHA = 200;

/**
 * 页面那一组的每一格对应 MD3 的哪个角色：浅色、深色各一份（照「晨光」「tokyonight」的对应：浮起来的东西浅色里比底亮、深色里也比底亮一档）。
 * @type {Record<'light'|'dark', Record<string, string>>}
 */
const PAGE = {
  light: {
    surface: 'surface', sidebar_bg: 'surfaceContainer', surface_1: 'surfaceContainerLowest', surface_2: 'surfaceContainerHigh',
    surface_3: 'surfaceContainerHighest', sidebar_active: 'surfaceContainerHigh',
  },
  dark: {
    surface: 'surface', sidebar_bg: 'surfaceContainerLowest', surface_1: 'surfaceContainerLow', surface_2: 'surfaceContainer',
    surface_3: 'surfaceContainerHigh', sidebar_active: 'surfaceContainer',
  },
};
/** 浅色、深色都一样的几格 */
const COMMON = {
  text: 'onSurface', text_soft: 'onSurfaceVariant', line: 'outlineVariant', line_strong: 'outline', accent: 'primary',
  accent_soft: 'primaryContainer', on_primary: 'onPrimary', on_primary_container: 'onPrimaryContainer', gold: 'secondary', ribbon: 'tertiary',
};
/** 终端那一组跟着主色的几格 */
const TUI = { accent: 'primary', workspace: 'primary' };

/**
 * 头像的主色：RGBA 的像素（`ImageData.data` 那样一格四个数），太透明的不算；一格都不剩的是 `null`。
 * @param {ArrayLike<number>} rgba
 * @returns {number|null} ARGB
 */
export function seedOf(rgba) {
  const pixels = [];
  for (let i = 0; i + 3 < rgba.length; i += 4) {
    if (rgba[i + 3] < MIN_ALPHA) continue;
    pixels.push(argbFromRgb(rgba[i], rgba[i + 1], rgba[i + 2]));
  }
  if (!pixels.length) return null;
  const ranked = Score.score(QuantizerCelebi.quantize(pixels, QUANTIZE));
  return ranked[0] ?? null;
}

/**
 * 照主色生成一套要盖的颜色。
 * @param {number|string} seed ARGB 或 `#rrggbb`（自选的主色）
 * @param {'light'|'dark'} scheme 选中的那一套是浅的还是深的
 * @returns {{page: Record<string, string>, tui: Record<string, string>}}
 */
export function overlayOf(seed, scheme) {
  const argb = typeof seed === 'string' ? argbFromHex(seed) : seed;
  const s = new SchemeTonalSpot(Hct.fromInt(argb), scheme === 'dark', 0);
  const pick = (/** @type {Record<string, string>} */ table) => Object.fromEntries(Object.entries(table).map(([k, role]) => [k, hexFromArgb(/** @type {any} */ (D)[role].getArgb(s))]));
  return { page: { ...pick(PAGE[scheme]), ...pick(COMMON) }, tui: pick(TUI) };
}

/** ARGB 写成 `#rrggbb`（给人看、记缓存）。 @param {number} argb */
export const hexOf = (argb) => hexFromArgb(argb);
