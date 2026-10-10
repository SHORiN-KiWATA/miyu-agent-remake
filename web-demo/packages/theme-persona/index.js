// @ts-check
//! 跟着人格的主题色（软件包 `theme-persona`，蓝图 `web.md`「主题」的「跟着人格的外观」）：照正在看的会话用的人格（状态事件
//! `look.changed`，和左栏左上角、对话区的头像同一份）的头像取主色，往主题包的 `theme.overlays` 挂一份要盖的颜色；浅色、深色照选中的
//! 那一套。人格没有头像、无人格的不挂，就是选中的那一套。停用了这个包就是原来的主题。
//!
//! 头像在浏览器里缩小到长边 `sample_side` 再取色（取色只看颜色的分布，不用原图那么大）；同一张图取过的记着，换回来不再算。

import { seedOf, overlayOf } from './model.js';

/** @param {any} ctx */
export function apply(ctx) {
  /** 头像的地址 → 主色（ARGB；取不出来的是 `null`） @type {Map<string, Promise<number|null>>} */
  const seeds = new Map();
  /** 挂着的那一份：撤掉用 @type {(() => void)|null} */
  let undo = null;
  /** 最近一次要的头像：取色是异步的，取完时已经换了人格的不挂 */
  let wanted = /** @type {string|null} */ (null);

  const unmount = () => {
    undo?.();
    undo = null;
  };
  /** @param {number} seed */
  const mount = (seed) => {
    unmount();
    undo = ctx.slots.mount('theme.overlays', { id: 'persona', render: (/** @type {'light'|'dark'} */ scheme) => overlayOf(seed, scheme) });
  };

  /** 读一张图、缩小、取色。 @param {string} url */
  const sample = (url) => new Promise((resolve) => {
    const img = new Image();
    img.decoding = 'async';
    img.onload = () => {
      const side = ctx.config.sample_side;
      const k = Math.min(1, side / Math.max(img.naturalWidth, img.naturalHeight));
      const w = Math.max(1, Math.round(img.naturalWidth * k));
      const h = Math.max(1, Math.round(img.naturalHeight * k));
      const canvas = document.createElement('canvas');
      canvas.width = w;
      canvas.height = h;
      const g = canvas.getContext('2d', { willReadFrequently: true });
      if (!g) return resolve(null);
      g.drawImage(img, 0, 0, w, h);
      try {
        resolve(seedOf(g.getImageData(0, 0, w, h).data));
      } catch (err) {
        console.error(`头像取不了色：${/** @type {any} */ (err)?.message ?? err}`);
        resolve(null);
      }
    };
    img.onerror = () => resolve(null);
    img.src = url;
  });

  ctx.on('look.changed', async (/** @type {{name: string, avatar?: string|null}|null} */ look) => {
    const url = look?.avatar ?? null;
    wanted = url;
    if (!url) return unmount();
    if (!seeds.has(url)) seeds.set(url, sample(url));
    const seed = await seeds.get(url);
    if (wanted !== url) return;
    if (seed == null) unmount();
    else mount(seed);
  });
  ctx.effect(() => () => unmount());
}
