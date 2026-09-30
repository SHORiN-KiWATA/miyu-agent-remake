// @ts-check
//! 代码块上色（蓝图 `web.md`「她的回答：Markdown」第 5 条，照旧版 `highlight.js`）。
//!
//! 只用 Prism 1.29 的分词（全局的 `Prism`，页面上 `data-manual` 载进来），不碰 `highlight()`、`highlightElement()`：
//! 那两个出 HTML 字符串，得走 `innerHTML`。这里自己造 span，模型写的字没有机会变成标签（第 1 条）。
//! 顺带：`after-tokenize` 钩子只有 `highlight()` 跑，Prism 的 markdown 语法在那里把围栏里的代码换成 HTML 字符串，
//! 走 `tokenize()` 碰不到（旧版实测复核）。
//!
//! 三道保险，哪一道不过都照纯文字：缺一块颜色无所谓，缺一个字不行。
//! - 没写语言、明说是纯文字的、不认识的、分词抛错的，不上色。
//! - 上色以后的字和原文一个一个比，对不上的整块不上色（语法定义吞字、换字，宁可不上色）。
//! - 超过 `max_chars` 的不上色：分词是同步的，Prism 的正则在病态输入上回溯，一屏卡死。
//!
//! 在收的代码（围栏没收齐，或者在收的回答里最后那一块）先照纯文字，停 `settle_ms` 没有新字再上色：半截的代码每来一段
//! 就分一次词既白干又闪（一个没写完的字符串把后面整段吞成字符串，下一段又变回来）。回答每来一段整个重画（`ui/chat.js`），
//! 所以这里只记一个页面上的钟：每画一次在收的代码就重新计时，到点时给页面上还挂着「待上色」的代码上色。
//! 收齐了的代码当场上色；上过的记着（`cache_size` 块），重画时不再分词。

import { res } from '../util/res.js';
import { h, replace } from '../ui/dom.js';

/**
 * 上好色的树：字，或者一段颜色（`role` 是 `tok-*` 的那个名字，里面还是树）。
 * @typedef {string | {role: string, kids: Painted[]}} Painted
 */

/** 分过的词：`语言\n原文` → 树，不上色的是 `null`。 */
const cache = new Map();
/** 在收的代码停多久再上色的钟。 */
let timer = 0;

/**
 * 分词上色：Prism 的 token 树换成 [`Painted`]；不上色的（见开头的三道保险）是 `null`。纯的，不碰 DOM。
 * @param {string} text 代码
 * @param {string} lang 围栏上写的语言
 * @returns {Painted[]|null}
 */
export function tokens(text, lang) {
  const cfg = res.markdown.highlight;
  const prism = globalThis.Prism;
  if (typeof prism?.tokenize !== 'function' || !text || text.length > cfg.max_chars) return null;
  const grammar = grammarFor(prism, lang);
  if (!grammar) return null;
  const key = `${lang}\n${text}`;
  if (cache.has(key)) return cache.get(key);
  const tree = split(prism, text, grammar);
  const result = tree && plain(tree) === text ? tree : null;
  if (text.length <= cfg.cache_max_chars) {
    if (cache.size >= cfg.cache_size) cache.delete(cache.keys().next().value);
    cache.set(key, result);
  }
  return result;
}

/**
 * 给一个装着代码原文的 `<code>` 上色。`settled` 是这块代码定了没有：定了的当场上，没定的挂上「待上色」，
 * 停一会儿没有新的画再上（见开头）。上不了色的照原文不动。
 * @param {HTMLElement} code
 * @param {string} lang
 * @param {string} text
 * @param {boolean} settled
 */
export function paint(code, lang, text, settled) {
  if (settled) {
    fill(code, tokens(text, lang));
    return;
  }
  if (!lang) return;
  code.dataset.hlPending = lang;
  clearTimeout(timer);
  timer = setTimeout(settle, res.markdown.highlight.settle_ms);
}

/**
 * 上好色的树 → 节点（span 加字）。别处要照同一套上色的（以后预览工作区的代码）用它。
 * @param {Painted[]} tree
 * @returns {(Node|string)[]}
 */
export function spans(tree) {
  return tree.map((n) => (typeof n === 'string' ? n : h(`span.tok-${n.role}`, spans(n.kids))));
}

/** 到点：页面上还挂着的「待上色」都上。已经被重画换掉的不在页面上，自然不管。 */
function settle() {
  for (const code of document.querySelectorAll('code[data-hl-pending]')) {
    const el = /** @type {HTMLElement} */ (code);
    const lang = el.dataset.hlPending ?? '';
    delete el.dataset.hlPending;
    fill(el, tokens(el.textContent ?? '', lang));
  }
}

function fill(code, tree) {
  if (tree) replace(code, spans(tree));
}

/** 语言 → Prism 的语法：先照别名换名字；明说是纯文字的、Prism 没有的是 `null`。名单里的键只认自己的，不走原型。 */
function grammarFor(prism, lang) {
  const { plain: plainLangs, aliases } = res.markdown.highlight;
  const raw = String(lang ?? '').trim().toLowerCase();
  if (!raw || plainLangs.includes(raw)) return null;
  const name = Object.hasOwn(aliases, raw) ? aliases[raw] : raw;
  const grammar = prism.languages && Object.hasOwn(prism.languages, name) ? prism.languages[name] : null;
  return grammar && typeof grammar === 'object' ? grammar : null;
}

/** 分词。抛错的照不上色处理（第一道保险），不往上抛：少一块颜色，正文照样在。 */
function split(prism, text, grammar) {
  let stream;
  try {
    stream = prism.tokenize(text, grammar);
  } catch {
    return null;
  }
  const out = [];
  flatten(stream, out);
  return out;
}

/**
 * Prism 的 token 树 → [`Painted`]。内容只会是字、数组、一个 token 三种；别的当字兜底。
 * 没对上颜色的 token 不多包一层。
 */
function flatten(content, out) {
  if (typeof content === 'string') {
    if (content) out.push(content);
  } else if (Array.isArray(content)) {
    for (const item of content) flatten(item, out);
  } else if (content && typeof content === 'object' && 'content' in content) {
    const role = roleOf(content);
    if (!role) {
      flatten(content.content, out);
      return;
    }
    const kids = [];
    flatten(content.content, kids);
    out.push({ role, kids });
  } else {
    out.push(String(content ?? ''));
  }
}

/** token 自己的种类先对，对不上再看 alias（差异的 `deleted-sign` 只能靠 alias）。 */
function roleOf(token) {
  const { roles } = res.markdown.highlight;
  const role = (name) => (typeof name === 'string' && Object.hasOwn(roles, name) ? roles[name] : '');
  if (role(token.type)) return role(token.type);
  const aliases = Array.isArray(token.alias) ? token.alias : [token.alias];
  for (const name of aliases) if (role(name)) return role(name);
  return '';
}

/** 树里的字连起来：和原文比（第二道保险）。 */
function plain(tree) {
  return tree.map((n) => (typeof n === 'string' ? n : plain(n.kids))).join('');
}
