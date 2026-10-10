// @ts-check
//! 跟着人格的外观（软件包 `theme-persona`，蓝图 `web.md`「主题」的「跟着人格的外观」）：照正在看的会话用的人格（状态事件
//! `look.changed`，和左栏左上角、对话区的头像同一份）：
//! - 主题色：人格自选了的（核心 P-6 的 `seed`）照它，没选的照头像取主色；往主题包的 `theme.overlays` 挂一份要盖的颜色，浅色、深色照
//!   选中的那一套。都没有、无人格的不挂，就是选中的那一套。
//! - 背景图（核心 P-6）：铺在对话区（`style.css`：页面根上 `has-persona-backdrop`、变量 `--persona-backdrop`），上面盖一层底色，多浓是
//!   设置项 `veil`。左栏照旧用主题色。
//! 停用了这个包就是原来的主题、没有背景图。
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
  /** @param {number|string} seed ARGB（从头像取的）或 `#rrggbb`（自选的） */
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

  const root = document.documentElement;
  /** 铺着的背景图（改了设置项 `veil` 照它重铺） */
  let shown = /** @type {string|null} */ (null);
  /** 背景图：有的铺上、没有的拿掉 @param {string|null} url */
  const backdrop = (url) => {
    shown = url;
    root.classList.toggle('has-persona-backdrop', !!url);
    if (url) {
      root.style.setProperty('--persona-backdrop', `url("${url}")`);
      root.style.setProperty('--persona-veil', `${Math.round(ctx.config.veil * 100)}%`);
    } else {
      root.style.removeProperty('--persona-backdrop');
      root.style.removeProperty('--persona-veil');
    }
  };

  ctx.on('look.changed', async (/** @type {{name: string, avatar?: string|null, seed?: string|null, background?: string|null}|null} */ look) => {
    backdrop(look?.background ?? null);
    // 自选的主题色照它，不用取
    if (look?.seed) {
      wanted = look.seed;
      return mount(look.seed);
    }
    const url = look?.avatar ?? null;
    wanted = url;
    if (!url) return unmount();
    if (!seeds.has(url)) seeds.set(url, sample(url));
    const seed = await seeds.get(url);
    if (wanted !== url) return;
    if (seed == null) unmount();
    else mount(seed);
  });
  // 盖的那一层多浓改了：当场重铺
  ctx.watchConfig(() => backdrop(shown));
  ctx.effect(() => () => {
    unmount();
    backdrop(null);
  });
}
