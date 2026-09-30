// @ts-check
//! 块和行内节点 → DOM（蓝图 `web.md`「她的回答：Markdown」第 1、4、5 条，照旧版 `app.js:4271-4390`、`4700-4839`、
//! `4949-5076` 造的样子）。只用 `h()`（`createElement`、字节点），不走 `innerHTML`：她写的字没有机会变成标签。
//! 类名照旧版，`styles/markdown.css` 照它画。

import { h, icon } from '../ui/dom.js';
import { t } from '../util/res.js';
import { filePath } from './inline.js';
import { math } from './math.js';
import { paint } from './highlight.js';
import { writeClipboard } from '../core/host.js';

/**
 * 造 DOM 时带着的：扩展点（`render.js` 的 `Hooks`）和提示（复制成了没有）。
 * @typedef {{hooks: import('./render.js').Hooks, say?: (text: string, good?: boolean) => void}} Context
 */

/**
 * 一串块 → 一个片段。
 * @param {import('./parse.js').Block[]} blocks
 * @param {Context} ctx
 * @returns {DocumentFragment}
 */
export function build(blocks, ctx) {
  const frag = document.createDocumentFragment();
  for (const b of blocks) frag.append(block(b, ctx));
  return frag;
}

/** @param {import('./parse.js').Block} b @param {Context} ctx @returns {Node|string} */
function block(b, ctx) {
  switch (b.type) {
    case 'code':
      return ctx.hooks.code?.(b.lang, b.text, b.closed) ?? codeBlock(b, ctx);
    case 'line':
      return b.value;
    case 'math':
      return h('div.math-block', math(b.tex, true, b.raw));
    case 'table':
      return table(b, ctx);
    case 'hr':
      return h('hr');
    case 'heading':
      return h(`h${b.level}`, kids(b.kids, ctx));
    case 'list':
      return h(b.ordered ? 'ol' : `ul${b.task ? '.task-list' : ''}`, b.items.map((it) => (it.check == null
        ? h('li', kids(it.kids, ctx))
        : h('li.task-list-item', h('input', { type: 'checkbox', checked: it.check, disabled: true }), h('span', kids(it.kids, ctx))))));
    case 'quote':
      return h('blockquote', kids(b.kids, ctx));
    default:
      return h('p', kids(b.kids, ctx));
  }
}

/**
 * 代码块：头一行语言（没写的写「代码」）和复制按钮，下面是代码；上色见 `highlight.js`。
 * @param {{lang: string, text: string, closed: boolean}} b
 * @param {Context} ctx
 */
export function codeBlock(b, ctx) {
  const code = h('code', b.lang ? { class: `language-${b.lang}` } : null, b.text);
  paint(code, b.lang, b.text, b.closed);
  const label = t('markdown.copy_code');
  return h('div.code-block',
    h('div.code-toolbar',
      h('span', b.lang || t('markdown.code')),
      h('button.code-copy-button', { type: 'button', title: label, 'aria-label': label, onclick: () => copy(b.text, ctx.say) }, icon('copy'))),
    h('pre', code));
}

/** 表格：外面一层自己横着滚；对齐照分隔行。 */
function table(b, ctx) {
  const cell = (tag, content, k) => h(`${tag}${b.align[k] ? `.align-${b.align[k]}` : ''}`, tag === 'th' ? { scope: 'col' } : null, kids(content, ctx));
  return h('div.markdown-table-scroll', h('table',
    h('thead', h('tr', b.head.map((c, k) => cell('th', c, k)))),
    b.rows.length ? h('tbody', b.rows.map((r) => h('tr', r.map((c, k) => cell('td', c, k))))) : null));
}

/** @param {import('./inline.js').Inline[]} nodes @param {Context} ctx @returns {any[]} 节点、字，`h()` 摊平 */
function kids(nodes, ctx) {
  return nodes.map((n) => inlineNode(n, ctx));
}

/** @param {import('./inline.js').Inline} n @param {Context} ctx @returns {any} 一个节点、字，或者几个（`h()` 摊平） */
function inlineNode(n, ctx) {
  if (typeof n === 'string') return n;
  switch (n.type) {
    case 'br':
      return h('br');
    case 'code':
      return h('code', n.text);
    case 'math':
      return math(n.tex, false, n.raw);
    case 'link':
      return link(n.href, '', kids(n.kids, ctx), ctx);
    case 'autolink':
      return link(n.href, '.auto-link', [n.text], ctx);
    case 'image':
      // 没接扩展点、扩展点不认的（比如地址是个 mailto:）照原文写
      return ctx.hooks.image?.(n.src, n.alt) ?? `![${n.alt}](${n.src})`;
    case 'titled':
      // 标题和地址在同一个链接里，点哪儿都能走；地址那半截退半档（旧版 `.link-url`）
      return [n.indent || null, link(n.href, '.auto-link.title-link', [
        h('span.link-title', kids(n.title, ctx)), `${n.gap}${n.open}`, h('span.link-url', n.url), n.close,
      ], ctx)];
    default:
      return h(n.type, kids(n.kids, ctx));
  }
}

/**
 * 一个链接：新标签页打开。`file://` 的点一下复制路径、悬停看全路径（第 4 条）：浏览器不让从网页跳到本机文件，
 * 做成真链接点了什么都不会发生，比不成链更让人糊涂（旧版 `app.js:4271-4290`）。
 */
function link(href, classes, content, ctx) {
  if (!/^file:/i.test(href)) return h(`a${classes}`, { href, rel: 'noopener noreferrer', target: '_blank' }, content);
  const path = filePath(href);
  return h(`a${classes}.path-link`, {
    href, rel: 'noopener noreferrer', title: path,
    onclick: (/** @type {Event} */ e) => { e.preventDefault(); copy(path, ctx.say); },
  }, content);
}

/**
 * 复制，提示复制了几个字（照会话编号的复制，`ui/panel.js`）；复制不了的提示原因。
 * @param {string} text
 * @param {((text: string, good?: boolean) => void)|undefined} say
 */
export async function copy(text, say) {
  try {
    await writeClipboard(text);
    say?.(t('copied', { count: [...text].length }), true);
  } catch (err) {
    if (say) say(t('copy_failed', { reason: err.message }));
    else console.error(err);
  }
}
