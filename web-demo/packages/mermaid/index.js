// @ts-check
//! mermaid 图（软件包 `mermaid`，蓝图 `web.md`「mermaid 图」）：挂进 Markdown 的按键分派挂载位 `markdown.code`，键是 `mermaid`；
//! 停用了没人接这个键，照代码块写（拿掉不留坑）。画过的图照源码记在这个包的这一次加载里，同一份整页只问一次桥。

import { mermaidBlock } from './mermaid.js';

/** @param {any} ctx */
export function apply(ctx) {
  /** 源码 → 画好的 SVG 文字（画不出来的是 `null`） */
  const drawn = new Map();
  const draw = (source) => {
    if (!drawn.has(source)) {
      drawn.set(source, ctx.core.request('web.mermaid', { source }).then((r) => r?.svg ?? null).catch((err) => {
        console.error(`mermaid 画不出来：${err.message}`);
        return null;
      }));
    }
    return drawn.get(source);
  };
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
