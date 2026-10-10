// @ts-check
//! 第一次引导（软件包 `welcome`，蓝图 `web.md`「第一次引导」）：连上核心以后读 `ui.welcomed`，是 `false` 的整页盖上引导（`guide.js`）；
//! 核心没有这一项（旧核心）的不出。换了界面语言重载过页面的，接着从记着的那一步走（`welcome.step`，读了就删：中途关了页面下次从头来）。
//! 这个包排在对话页（`app`）前面加载：这台设备上还没记着「走过了」（`welcome.done`）的，一起来就盖一层底色，读到 `ui.welcomed` 再定是
//! 开引导还是揭掉（2026-10-10 项目主人：刚打开时会先露一下空会话）。

import { Guide } from './guide.js';
import { STEP_KEY, DONE_KEY } from './screens.js';

/** @param {any} ctx */
export function apply(ctx) {
  /** @type {Guide|null} */
  let guide = null;
  // 一层底色：对话页画出来以前就盖上（内联的样式，不等样式表），引导开了它垫在下面，揭掉时淡出
  const cover = ctx.storage.get(DONE_KEY, false) ? null : document.body.appendChild(Object.assign(document.createElement('div'), {
    className: 'wl-cover',
    style: 'position: fixed; z-index: 37; inset: 0; background: var(--surface, #faf9f7); transition: opacity 0.2s ease',
  }));
  // 吉祥物先藏着：盖着底色、还没定站哪时不在输入框上露一下（引导给了它舞台、或者揭掉底色时放开）
  if (cover) document.body.dataset.mascot = 'hold';
  const release = () => {
    if (document.body.dataset.mascot === 'hold') delete document.body.dataset.mascot;
  };
  const uncover = () => {
    release();
    if (!cover) return;
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
    guide = new Guide(ctx, uncover, release);
    guide.open(typeof step === 'string' ? step : 'hello');
  };
  start();
  ctx.effect(() => () => {
    guide?.close();
    cover?.remove();
    release();
  });
}
