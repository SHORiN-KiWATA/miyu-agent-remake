// @ts-check
//! 公式（蓝图 `web.md`「她的回答：Markdown」第 6 条，照旧版 `app.js:4893-4908`）：KaTeX 0.18.4 排版（全局的 `katex`，
//! 页面上载进来）。`throwOnError: false`：写错的公式 KaTeX 自己标红排出来；KaTeX 没载进来、还是抛了错的，照原文写。
//! 在收的时候没收齐的定界符本来就照原文（`parse.js`、`inline.js` 认不出公式），收齐了的下一次重画自己变公式。

import { h } from '../ui/dom.js';

/**
 * 一个公式的节点。
 * @param {string} tex 公式的字（不带定界符）
 * @param {boolean} display 独占几行的（`div.math-display`），还是行内的（`span.math-inline`）
 * @param {string} raw 原文（带定界符）：排不了时照它写
 * @returns {Node}
 */
export function math(tex, display, raw) {
  const katex = /** @type {any} */ (globalThis).katex;
  const trimmed = tex.trim();
  if (trimmed && typeof katex?.render === 'function') {
    const node = h(display ? 'div.math-display' : 'span.math-inline');
    try {
      katex.render(trimmed, node, { displayMode: display, throwOnError: false, strict: 'ignore' });
      return node;
    } catch (err) {
      // throwOnError 关着还抛的，是 KaTeX 自己排不了（不是写错了）：照原文写，原因记在控制台
      console.warn(`公式排不了，照原文写：${err.message}`);
    }
  }
  return document.createTextNode(raw);
}
