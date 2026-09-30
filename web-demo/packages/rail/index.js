// @ts-check
//! 右边的跳转条（软件包 `rail`，蓝图 `web.md`「右边的跳转条」）：挂进对话区右边的挂载位 `stage.right`，照对话区（服务 `chat`）
//! 交来的你说的话画。停用了右边什么都没有，别的照常。

import { PromptRail } from './rail.js';

/** @param {any} ctx */
export function apply(ctx) {
  ctx.slots.mount('stage.right', {
    id: 'rail',
    order: 10,
    render: () => {
      const rail = new PromptRail(ctx.chat.scroller, ctx.chat.list, ctx.config);
      const off = ctx.chat.onPrompts((prompts) => rail.update(prompts));
      return { el: rail.el, dispose: () => { off(); rail.dispose(); } };
    },
  });
}
