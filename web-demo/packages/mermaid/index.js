// @ts-check
//! mermaid 图（软件包 `mermaid`，蓝图 `web.md`「mermaid 图」）：挂进 Markdown 的按键分派挂载位 `markdown.code`，键是 `mermaid`；
//! 停用了没人接这个键，照代码块写（拿掉不留坑）。画过的图照源码记在这个包的这一次加载里，同一份整页只问一次核心。

import { mermaidBlock } from './mermaid.js';
import { drawer } from './draw.js';

/** @param {any} ctx */
export function apply(ctx) {
  // 问核心画（`mermaid.render`），回的记号色换成设置项 `colors` 里的页面颜色
  const draw = drawer((method, params) => ctx.core.request(method, params), () => ctx.config.colors, (m) => console.error(m));
  const deps = {
    draw,
    codeBlock: ctx.markdown.codeBlock,
    copy: ctx.markdown.copy,
    // 用的时候再找灯箱：没装的经宿主在外面开（浏览器是新标签页，桌面端是系统的浏览器）
    open: (what) => (ctx.lightbox ? ctx.lightbox.open(what) : ctx.host.open(what.url)),
    t: (path, fields) => ctx.text(path, fields),
  };
  // 图最高多少：设置项，写成 CSS 变量给样式用
  ctx.effect(() => {
    document.documentElement.style.setProperty('--mermaid-max', `${ctx.config.max_height}px`);
    return () => document.documentElement.style.removeProperty('--mermaid-max');
  });
  ctx.slots.mount('markdown.code', {
    id: 'mermaid',
    key: 'mermaid',
    render: ({ text, say }) => mermaidBlock(text, say, deps),
  });
}
