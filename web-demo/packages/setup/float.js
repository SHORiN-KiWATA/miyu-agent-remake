// @ts-check
//! 浮起来的卡片（人格、预设的详情和新建，2026-10-09 项目主人：原地展开的动画和样子都不好，改成悬浮卡片）：盖住整个设置框
//! （`.set-panel`），底下一层半透明糊一下，正中一张卡片：头一行标题和 ✕，中间的内容能滚。进出照「动效」：淡入、从 0.97 放大；
//! 收起淡出。点卡片外面、`Esc`（设置页的 `set-dismiss`）、✕ 都交给 `onDismiss`（改了没存的由编辑器自己决定收不收）。

import { h, icon, replace } from '../../src/lib/dom.js';
import { leave } from '../../src/lib/motion.js';

/**
 * @param {{title: Node|string, body: Node, close: string, onDismiss: () => void}} o
 */
export function floatCard(o) {
  const title = h('h3.setup-float-title', o.title);
  const body = h('div.setup-float-body', o.body);
  const card = h('div.setup-float', { role: 'dialog', 'aria-modal': 'true' },
    h('div.setup-float-head', title, h('button.icon-button.setup-float-close', { type: 'button', title: o.close, 'aria-label': o.close, onclick: () => o.onDismiss() }, icon('x'))),
    body);
  const scrim = h('div.setup-float-scrim', { 'data-set-dismiss': '' }, card);
  scrim.addEventListener('pointerdown', (e) => { if (e.target === scrim) o.onDismiss(); });
  scrim.addEventListener('set-dismiss', () => o.onDismiss());
  return {
    el: scrim,
    /** 放进 `from` 所在的设置框里（找不到的放整页上）。 @param {HTMLElement} from */
    mount: (from) => (from.closest('.set-panel') ?? document.body).append(scrim),
    /** @param {Node|string} text */
    setTitle: (text) => replace(title, text),
    /** @param {Node} node */
    setBody: (node) => replace(body, node),
    /** 收起：淡出、拿掉。 */
    close: () => leave(scrim, () => scrim.remove()),
  };
}
