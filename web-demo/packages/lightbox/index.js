// @ts-check
//! 灯箱（软件包 `lightbox`）：提供服务 `lightbox`（`open(what)`）。用它的包都是用的时候再找（`lightbox~`、`lightbox?`）；
//! 停用了点图在新标签页开，别的照常。

import { Lightbox } from './lightbox.js';

/** @param {any} ctx */
export function apply(ctx) {
  const box = new Lightbox((path) => ctx.text(path));
  ctx.effect(() => () => box.destroy());
  ctx.provide('lightbox', { open: (what) => box.open(what) });
}
