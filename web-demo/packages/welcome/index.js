// @ts-check
//! 第一次引导（软件包 `welcome`，蓝图 `web.md`「第一次引导」）：连上核心以后读 `ui.welcomed`，是 `false` 的整页盖上引导（`guide.js`）；
//! 核心没有这一项（旧核心）的不出。换了界面语言重载过页面的，接着从记着的那一步走（`welcome.step`，读了就删：中途关了页面下次从头来）。

import { Guide } from './guide.js';
import { STEP_KEY } from './screens.js';

/** @param {any} ctx */
export function apply(ctx) {
  /** @type {Guide|null} */
  let guide = null;
  const start = async () => {
    let got;
    try {
      got = await ctx.core.request('config.get', { keys: ['ui.welcomed'] });
    } catch {
      return;
    }
    if (got?.items?.['ui.welcomed']?.value !== false) return;
    const step = ctx.storage.get(STEP_KEY, null);
    ctx.storage.set(STEP_KEY, null);
    guide = new Guide(ctx);
    guide.open(typeof step === 'string' ? step : 'hello');
  };
  start();
  ctx.effect(() => () => guide?.close());
}
