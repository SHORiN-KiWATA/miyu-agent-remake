// @ts-check
//! 行内的写法 → 一串节点（蓝图 `web.md`「她的回答：Markdown」第 3 条，照旧版 `app.js:4246-4578`）。纯函数，不碰 DOM：
//! 节点交给 `build.js` 造 DOM，这里测得了。
//!
//! 照这个先后认：独占一行的「标题（地址）」；`\(…\)`、`$$…$$`、`$…$` 公式；反斜杠转义；换行；`` `代码` ``；
//! `![说明](地址)` 图片（旧版没有，蓝图「图片」第 1 条加的）；`[字](地址)`；不带属性的 `<br>`；`<地址>`；裸地址；`~~删除~~`；`**粗**`、`__粗__`；`*斜*`、`_斜_`。
//! 认不出的照字。嵌套最多 `markdown.json` 的 `inline_depth` 层，再深的整段照字。
//!
//! 节点：字是字符串（挨着的并成一段）；别的是 `{type, …}`，见 [`Inline`]。

import { res } from '../util/res.js';

/**
 * 一个行内节点。
 * @typedef {string
 *   | {type: 'br'}
 *   | {type: 'code', text: string}
 *   | {type: 'math', tex: string, raw: string}
 *   | {type: 'link', href: string, kids: Inline[]}
 *   | {type: 'autolink', href: string, text: string}
 *   | {type: 'image', src: string, alt: string}
 *   | {type: 'titled', href: string, indent: string, title: Inline[], gap: string, open: string, url: string, close: string}
 *   | {type: 'del' | 'strong' | 'em', kids: Inline[]}} Inline
 *   `math` 的 `raw` 是原文：KaTeX 排不了时照它写（第 6 条）；`autolink` 是 `<地址>` 和裸地址，字照原样；
 *   `titled` 是「标题（地址）」一整行，标题、括号、地址都在同一个链接里
 */

/** 只认这几种地址（第 3 条）。`file` 在里面是因为模型会拿它指本机路径（旧版 `app.js:4246`）。 */
const SCHEME = /^(?:https?|file):\/\//i;
const PROTOCOLS = ['http:', 'https:', 'file:'];
/** 反斜杠后面照字写的几个。 */
const ESCAPES = '\\`*_[]|~$';
/** 不带属性的 `<br>`、`<br/>`、`<br />`，大小写不论；粘着从游标处试。带属性的、别的标签照字（第 1 条）。 */
const BREAK_TAG = /<br[ \t]*\/?>/iy;
/**
 * 裸地址：中日韩的标点一个都不进地址（「…archlinux.org、AUR」里顿号后面还有字母，只修末尾不够，
 * 整段会被 `new URL` 当成域名；旧版实测）。汉字本身不排除：维基那种中文路径是合法的。
 */
const BARE_URL = /(?:https?|file):\/\/[^\s<>"'`  -⁯　-〿＀-￯]+/iy;
/** 「标题（地址）」独占一行（旧版 `app.js:4352-4373`，和终端 `src/render/link.rs` 同一套）。 */
const TITLE_URL_LINE = /^([ \t]*)(\S[^\n]*?)([ \t]*)([（(])[ \t]*((?:https?|file):\/\/[^\s)）]+)[ \t]*([)）])[ \t]*$/;

/**
 * 认得的地址规范化以后的样子；不认的是 `null`。
 * @param {string} value
 * @returns {string|null}
 */
export function validLinkUrl(value) {
  const raw = String(value ?? '').trim();
  if (!SCHEME.test(raw)) return null;
  // 解析不了的地址就是不认的地址：照字写，不算出错
  if (!URL.canParse(raw)) return null;
  const url = new URL(raw);
  return PROTOCOLS.includes(url.protocol) ? url.href : null;
}

/**
 * `file://` 地址里的本机路径：转义解开；解不开的照原样（第 4 条：点一下复制它、悬停看它）。
 * @param {string} href
 */
export function filePath(href) {
  const path = String(href).replace(/^file:\/\//i, '');
  try {
    return decodeURIComponent(path) || String(href);
  } catch {
    // 地址里有不成对的 `%`：解不开就照写，路径照样能看、能复制
    return path;
  }
}

/**
 * 一段行内的字 → 节点。
 * @param {string} source
 * @param {number} [depth] 第几层：嵌套的写法里每进一层加一
 * @param {boolean} [inLink] 在链接的字里：不再认裸地址、`<地址>`、「标题（地址）」，链接里不套链接
 * @returns {Inline[]}
 */
export function inline(source, depth = 0, inLink = false) {
  const text = String(source ?? '');
  if (depth > res.markdown.inline_depth) return text ? [text] : [];
  /** @type {Inline[]} */
  const out = [];
  let i = 0;
  let plain = 0;
  /** 游标前面没认出来的字照写，接上认出来的节点，跳到 `next`。 */
  const emit = (node, next) => {
    if (i > plain) push(out, text.slice(plain, i));
    push(out, node);
    i = next;
    plain = next;
  };
  const deeper = (s) => inline(s, depth + 1, inLink);
  while (i < text.length) {
    const c = text[i];
    if ((i === 0 || text[i - 1] === '\n') && !inLink) {
      const hit = titleUrlLine(text, i);
      if (hit) {
        const { href, indent, gap, open, url, close } = hit;
        emit({ type: 'titled', href, indent, title: inline(hit.title, depth + 1, true), gap, open, url, close }, i + hit.length);
        continue;
      }
    }
    if (c === '\\' && text[i + 1] === '(') {
      const end = text.indexOf('\\)', i + 2);
      if (end > i + 1) {
        emit({ type: 'math', tex: text.slice(i + 2, end), raw: text.slice(i, end + 2) }, end + 2);
        continue;
      }
    }
    if (c === '\\' && i + 1 < text.length && ESCAPES.includes(text[i + 1])) {
      emit(text[i + 1], i + 2);
      continue;
    }
    if (c === '$') {
      const end = mathEnd(text, i);
      if (end > 0) {
        const wide = text[i + 1] === '$' ? 2 : 1;
        emit({ type: 'math', tex: text.slice(i + wide, end), raw: text.slice(i, end + wide) }, end + wide);
        continue;
      }
    }
    if (c === '\n') {
      emit({ type: 'br' }, i + 1);
      continue;
    }
    if (c === '`') {
      const end = text.indexOf('`', i + 1);
      if (end > i + 1) {
        emit({ type: 'code', text: text.slice(i + 1, end) }, end + 1);
        continue;
      }
    }
    // 图片的地址照写，不挑协议：本机的路径（相对的、~/ 的）由扩展点换成桥的地址（build.js 的 hooks.image）
    if (c === '!' && text[i + 1] === '[') {
      const labelEnd = text.indexOf('](', i + 2);
      const urlEnd = labelEnd >= 0 ? text.indexOf(')', labelEnd + 2) : -1;
      const src = urlEnd > labelEnd + 2 ? text.slice(labelEnd + 2, urlEnd).trim() : '';
      if (labelEnd >= 0 && src && !/\s/.test(src)) {
        emit({ type: 'image', src, alt: text.slice(i + 2, labelEnd) }, urlEnd + 1);
        continue;
      }
    }
    if (c === '[') {
      const labelEnd = text.indexOf('](', i + 1);
      const urlEnd = labelEnd >= 0 ? text.indexOf(')', labelEnd + 2) : -1;
      const href = labelEnd > i + 1 && urlEnd > labelEnd + 2 ? validLinkUrl(text.slice(labelEnd + 2, urlEnd)) : null;
      if (href) {
        emit({ type: 'link', href, kids: inline(text.slice(i + 1, labelEnd), depth + 1, true) }, urlEnd + 1);
        continue;
      }
    }
    // <br> 排在 ` 后面：行内代码里的 <br> 照字
    if (c === '<') {
      BREAK_TAG.lastIndex = i;
      const br = BREAK_TAG.exec(text);
      if (br) {
        emit({ type: 'br' }, i + br[0].length);
        continue;
      }
    }
    // <地址> 和裸地址排在 ` 和 [](…) 后面：行内代码和 Markdown 链接先被认走，这里看不到它们里面的字
    if (c === '<' && !inLink) {
      const end = text.indexOf('>', i + 1);
      const href = end > i + 1 ? validLinkUrl(text.slice(i + 1, end)) : null;
      if (href) {
        emit({ type: 'autolink', href, text: text.slice(i + 1, end) }, end + 1);
        continue;
      }
    }
    if ('hHfF'.includes(c) && !inLink) {
      const bare = bareUrlAt(text, i);
      if (bare) {
        emit({ type: 'autolink', href: bare.href, text: bare.raw }, i + bare.raw.length);
        continue;
      }
    }
    if (text.startsWith('~~', i)) {
      const end = text.indexOf('~~', i + 2);
      if (end > i + 2 && text.slice(i + 2, end).trim()) {
        emit({ type: 'del', kids: deeper(text.slice(i + 2, end)) }, end + 2);
        continue;
      }
    }
    const strong = text.startsWith('**', i) ? '**' : text.startsWith('__', i) ? '__' : null;
    if (strong && !(strong === '__' && isWordChar(text[i - 1]))) {
      const end = strong === '__' ? underscoreCloser(text, i + 2, '__') : text.indexOf('**', i + 2);
      if (end > i + 2 && text.slice(i + 2, end).trim()) {
        emit({ type: 'strong', kids: deeper(text.slice(i + 2, end)) }, end + 2);
        continue;
      }
    }
    if (c === '*' || (c === '_' && !isWordChar(text[i - 1]))) {
      const end = c === '_' ? underscoreCloser(text, i + 1, '_') : text.indexOf('*', i + 1);
      if (end > i + 1 && text.slice(i + 1, end).trim()) {
        emit({ type: 'em', kids: deeper(text.slice(i + 1, end)) }, end + 1);
        continue;
      }
    }
    i += 1;
  }
  if (text.length > plain) push(out, text.slice(plain));
  return out;
}

/** 接上一个节点：字和前面的字并成一段。 */
function push(out, node) {
  const last = out.length - 1;
  if (typeof node === 'string' && typeof out[last] === 'string') out[last] += node;
  else out.push(node);
}

/**
 * `$` 开头的公式在哪收尾（右边那个 `$` 的位置），不是公式的是 -1。
 * `$$…$$` 找下一个 `$$`；`$…$` 里面不空、不跨行、两头不是空白，右边的 `$` 后面不跟数字（`$5 到 $10` 是价钱）。
 */
function mathEnd(text, i) {
  if (text[i + 1] === '$') return text.indexOf('$$', i + 2);
  const end = text.indexOf('$', i + 1);
  if (end <= i + 1) return -1;
  const inner = text.slice(i + 1, end);
  const ok = !inner.includes('\n') && !/^\s|\s$/.test(inner) && !/\d/.test(text[end + 1] ?? '');
  return ok ? end : -1;
}

/**
 * 从 `i` 起的一个裸地址：`{raw, href}`，不是的是 `null`。前一个字是字母数字的不认（`xhttps://` 这种粘着的）。
 */
function bareUrlAt(text, i) {
  if (i > 0 && /[A-Za-z0-9]/.test(text[i - 1])) return null;
  BARE_URL.lastIndex = i;
  const matched = BARE_URL.exec(text);
  if (!matched) return null;
  const raw = trimUrlTail(matched[0]);
  const href = raw ? validLinkUrl(raw) : null;
  return href ? { raw, href } : null;
}

/** 句尾的标点吐回去（「见 https://a.com。」的句号不是地址的）；右括号只在成对时留下：维基的地址本身就带括号。 */
function trimUrlTail(raw) {
  const { url_tail: tail, url_pairs: pairs } = res.markdown;
  let value = raw;
  while (value) {
    const last = value.at(-1);
    if (Object.hasOwn(pairs, last)) {
      const opens = value.split(pairs[last]).length - 1;
      const closes = value.split(last).length - 1;
      if (closes <= opens) break;
    } else if (!tail.includes(last)) break;
    value = value.slice(0, -1);
  }
  return value;
}

/**
 * 从 `i` 起的这一行是不是「标题（地址）」：是的话给出各段和整行多长。
 * 标题里再有地址的不算；结尾是 `]` 的是 `[字](地址)`，不拦。标题得是一句名字、不是一段话：
 * 有中文的句末标点、英文的句界（句点问号叹号、空格、大写或汉字）、太长的，只让地址成链（旧版 09-11 手机上实测，
 * 一整段正文被包成了一个链接）。
 */
function titleUrlLine(text, i) {
  const lineEnd = text.indexOf('\n', i);
  const line = lineEnd < 0 ? text.slice(i) : text.slice(i, lineEnd);
  const matched = TITLE_URL_LINE.exec(line);
  if (!matched) return null;
  const [, indent, title, gap, open, url, close] = matched;
  if (title.includes('://') || title.endsWith(']')) return null;
  if ([...title].length > res.markdown.title_max_chars || /[。！？；]/.test(title) || /[.!?]\s+[A-Z一-鿿]/.test(title)) return null;
  const href = validLinkUrl(url);
  return href ? { length: line.length, indent, title, gap, open, url, close, href } : null;
}

/** 字母、数字、下划线是词内的字：下划线的粗斜两头都不能挨着它们（CommonMark 的词内规矩）。 */
function isWordChar(ch) {
  return Boolean(ch) && /[\p{L}\p{N}_]/u.test(ch);
}

/** 下划线的收尾：后面紧跟着词内的字的不算，接着往后找。 */
function underscoreCloser(text, from, marker) {
  let end = text.indexOf(marker, from);
  while (end !== -1) {
    if (!isWordChar(text[end + marker.length])) return end;
    end = text.indexOf(marker, end + 1);
  }
  return -1;
}
