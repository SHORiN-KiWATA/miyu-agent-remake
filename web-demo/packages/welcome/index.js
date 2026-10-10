// @ts-check
//! 第一次引导（软件包 `welcome`，蓝图 `web.md`「第一次引导」）：连上核心以后读 `ui.welcomed`，是 `false` 的整页盖上引导（`guide.js`）；
//! 核心没有这一项（旧核心）的不出。换了界面语言重载过页面的，接着从记着的那一步走（`welcome.step`，读了就删：中途关了页面下次从头来）。
//! 这个包排在对话页（`app`）前面加载：一起来就盖一层底色、吉祥物先藏着，读到 `ui.welcomed` 再定是开引导还是揭掉（2026-10-10 项目主人：
//! 刚打开时会先露一下空会话、吉祥物）。这台设备记着「走过了」（`welcome.done`）的照样盖：核心可能说没走过（别处重置过、测引导），
//! 读到走过了当场揭掉、不淡出；没记着的淡出。

import { Guide } from './guide.js';
import { STEP_KEY, DONE_KEY } from './screens.js';

/** @param {any} ctx */
export function apply(ctx) {
  /** @type {Guide|null} */
  let guide = null;
  const done = ctx.storage.get(DONE_KEY, false) === true;
  // 一层底色：对话页画出来以前就盖上（内联的样式，不等样式表），引导开了它垫在下面
  const cover = document.body.appendChild(Object.assign(document.createElement('div'), {
    className: 'wl-cover',
    style: 'position: fixed; z-index: 37; inset: 0; background: var(--surface, #faf9f7); transition: opacity 0.2s ease',
  }));
  // 吉祥物先藏着：盖着底色、还没定站哪时不在输入框上露一下（引导给了它舞台、或者揭掉底色时放开）
  document.body.dataset.mascot = 'hold';
  const release = () => {
    if (document.body.dataset.mascot === 'hold') delete document.body.dataset.mascot;
  };
  /** 揭掉底色：记着走过的当场拿掉（平常打开不多一下淡出），引导走完、没记着的淡出 @param {boolean} [fade] */
  const uncover = (fade = !done) => {
    release();
    if (!cover.isConnected) return;
    if (!fade) return cover.remove();
    cover.style.opacity = '0';
    setTimeout(() => cover.remove(), 220);
  };
  const start = async () => {
    let got;
    try {
      got = await ctx.core.request('config.get', { keys: ['ui.welcomed'] });
    } catch {
      uncover();
      return;
    }
    const value = got?.items?.['ui.welcomed']?.value;
    if (value !== false) {
      if (value === true) ctx.storage.set(DONE_KEY, true);
      uncover();
      return;
    }
    const step = ctx.storage.get(STEP_KEY, null);
    ctx.storage.set(STEP_KEY, null);
    guide = new Guide(ctx, () => uncover(true), release);
    guide.open(typeof step === 'string' ? step : 'hello');
  };
  start();
  ctx.effect(() => () => {
    guide?.close();
    cover.remove();
    release();
  });
}
