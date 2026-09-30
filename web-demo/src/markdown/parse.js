// @ts-check
//! 她的回答 → 一串块（蓝图 `web.md`「她的回答：Markdown」第 2 条，照旧版 `app.js:4736-5076`）。纯函数，不碰 DOM：
//! 块交给 `build.js` 造 DOM，这里测得了。
//!
//! 照这个先后认：空行；围起来的代码（```` ``` ```` 加语言，`~~~` 不算）；认领的一行（`options.line`，以后的音视频卡片）；
//! 独占几行的公式 `$$…$$`、`\[…\]`；表格；分隔线；标题（`#` 是 h2，往下顺延）；无序列表（不嵌套，`[ ]`、`[x]` 是任务）；
//! 有序列表（不嵌套、不认起始数）；引用（里面只认行内的写法）；段落（软换行照换行）。块里的字交给 `inline.js`。

import { inline } from './inline.js';

/**
 * 一块。`code` 的 `closed` 是围栏收齐了没有：没收齐的代码到末尾为止（在收的时候，见 `stream.js`）；
 * `table` 的 `align` 一列一个（`left`、`center`、`right`，空的是没定）；列表的一项 `check` 是任务的勾（`null` 不是任务）；
 * `line` 是认领的一行，`value` 是认领时给的东西，原样放着。
 * @typedef {{type: 'para' | 'quote', kids: import('./inline.js').Inline[]}
 *   | {type: 'heading', level: number, kids: import('./inline.js').Inline[]}
 *   | {type: 'code', lang: string, text: string, closed: boolean}
 *   | {type: 'math', tex: string, raw: string}
 *   | {type: 'table', align: string[], head: import('./inline.js').Inline[][], rows: import('./inline.js').Inline[][][]}
 *   | {type: 'hr'}
 *   | {type: 'list', ordered: boolean, task: boolean, items: {check: boolean|null, kids: import('./inline.js').Inline[]}[]}
 *   | {type: 'line', text: string, value: any}} Block
 */

/** 围栏的开头：```` ``` ```` 加可有可无的语言，别的字不许有（`stream.js` 照同一个认）。 */
export const FENCE_OPEN = /^\s*```\s*([\w.+-]*)\s*$/;
/** 围栏的收尾：只有 ```` ``` ````。 */
export const FENCE_CLOSE = /^\s*```\s*$/;
/** 认的语言名（第 2 条）；不像的当没写。 */
const LANG = /^[\w.+-]{1,40}$/;
const HEADING = /^(#{1,6})\s+(.+)$/;
const BULLET = /^\s*[-*+]\s+(.+)$/;
const NUMBERED = /^\s*\d+[.)]\s+(.+)$/;
const TASK = /^\[([ xX])\]\s+(.*)$/;
const QUOTE = /^\s*>\s?(.*)$/;
const MATH_DELIMS = [['$$', '$$'], ['\\[', '\\]']];

/**
 * 解析一条回答。
 * @param {string} source
 * @param {{line?: (line: string) => any}} [options] `line` 认领独占一行的：给一个不是 `null` 的值，这一行就单独成
 *   一块（`{type: 'line', value}`），段落也在这里断开；每一行最多问一次
 * @returns {Block[]}
 */
export function parse(source, { line } = {}) {
  const lines = String(source ?? '').replace(/\r\n?/g, '\n').split('\n');
  const asked = new Map();
  /** 这一行有没有被认领：问过的记着，段落里问过的，轮到它当块时不再问。 */
  const claim = (i) => {
    if (!line) return null;
    if (!asked.has(i)) asked.set(i, line(lines[i]) ?? null);
    return asked.get(i);
  };
  /** @type {Block[]} */
  const blocks = [];
  let i = 0;
  while (i < lines.length) {
    const text = lines[i];
    if (!text.trim()) {
      i += 1;
      continue;
    }
    const fence = FENCE_OPEN.exec(text);
    if (fence) {
      const body = [];
      i += 1;
      while (i < lines.length && !FENCE_CLOSE.test(lines[i])) body.push(lines[i++]);
      const closed = i < lines.length;
      if (closed) i += 1;
      blocks.push({ type: 'code', lang: LANG.test(fence[1]) ? fence[1] : '', text: body.join('\n'), closed });
      continue;
    }
    const value = claim(i);
    if (value != null) {
      blocks.push({ type: 'line', text, value });
      i += 1;
      continue;
    }
    const math = /^\s*(\$\$|\\\[)/.test(text) ? mathBlock(lines, i) : null;
    if (math) {
      blocks.push({ type: 'math', tex: math.tex, raw: lines.slice(i, math.next).join('\n') });
      i = math.next;
      continue;
    }
    const tabled = isTableStart(lines, i) ? table(lines, i) : null;
    if (tabled) {
      blocks.push(tabled.block);
      i = tabled.next;
      continue;
    }
    if (isRule(text)) {
      blocks.push({ type: 'hr' });
      i += 1;
      continue;
    }
    const heading = HEADING.exec(text);
    if (heading) {
      blocks.push({ type: 'heading', level: Math.min(6, heading[1].length + 1), kids: inline(heading[2]) });
      i += 1;
      continue;
    }
    const listed = list(lines, i);
    if (listed) {
      blocks.push(listed.block);
      i = listed.next;
      continue;
    }
    if (/^\s*>/.test(text)) {
      const quoted = [];
      for (let m; i < lines.length && (m = QUOTE.exec(lines[i])); i += 1) quoted.push(m[1]);
      blocks.push({ type: 'quote', kids: inline(quoted.join('\n')) });
      continue;
    }
    const para = [text];
    i += 1;
    while (i < lines.length && lines[i].trim() && !startsBlock(lines, i, claim)) para.push(lines[i++]);
    blocks.push({ type: 'para', kids: inline(para.join('\n')) });
  }
  return blocks;
}

/** 这一行能不能开一块：段落碰到它就断开。 */
function startsBlock(lines, i, claim) {
  const text = lines[i];
  return /^\s*```/.test(text) || /^#{1,6}\s+/.test(text) || /^\s*[-*+]\s+/.test(text) || /^\s*\d+[.)]\s+/.test(text)
    || /^\s*>/.test(text) || isRule(text) || isTableStart(lines, i) || /^\s*\$\$/.test(text) || /^\s*\\\[\s*$/.test(text)
    || claim(i) != null;
}

/** 分隔线：三个以上的 `*`、`-`、`_`，中间可以有空白。 */
function isRule(text) {
  const t = text.trim();
  return /^(?:\*\s*){3,}$/.test(t) || /^(?:-\s*){3,}$/.test(t) || /^(?:_\s*){3,}$/.test(t);
}

/**
 * 独占几行的公式：`{tex, next}`（`next` 是公式后面那一行），没收齐的是 `null`（在收的时候照原文，收齐了下一次重画自己变公式）。
 * 开头那一行、收尾那一行可以带着公式的字；一行里开了又收的也算。
 */
function mathBlock(lines, index) {
  const first = lines[index].trim();
  for (const [open, close] of MATH_DELIMS) {
    if (!first.startsWith(open)) continue;
    const rest = first.slice(open.length);
    if (rest.length > close.length && rest.endsWith(close)) return { tex: rest.slice(0, -close.length), next: index + 1 };
    const body = rest && rest !== close ? [rest] : [];
    for (let i = index + 1; i < lines.length; i += 1) {
      const candidate = lines[i].trim();
      if (candidate.endsWith(close)) {
        if (candidate !== close) body.push(candidate.slice(0, -close.length));
        return { tex: body.join('\n'), next: i + 1 };
      }
      body.push(lines[i]);
    }
    return null;
  }
  return null;
}

/**
 * 表格的一行拆成格：`\|` 和代码里的 `|` 不分格；两头的 `|` 可有可无。
 * `separated` 是这一行里有没有分格的 `|`：没有的不算表格的一行。
 */
function row(line) {
  const text = String(line ?? '').trim();
  const cells = [];
  let cell = '';
  let fence = 0;
  let separated = false;
  let endsWithBar = false;
  for (let i = 0; i < text.length;) {
    if (text[i] === '\\' && i + 1 < text.length) {
      cell += text.slice(i, i + 2);
      i += 2;
    } else if (text[i] === '`') {
      let end = i + 1;
      while (end < text.length && text[end] === '`') end += 1;
      // 一样长的一串反引号才收尾：`` `a|b` `` 里的 | 不分格
      if (!fence) fence = end - i;
      else if (fence === end - i) fence = 0;
      cell += text.slice(i, end);
      i = end;
    } else if (text[i] === '|' && !fence) {
      cells.push(cell.trim());
      cell = '';
      separated = true;
      endsWithBar = true;
      i += 1;
      continue;
    } else {
      cell += text[i];
      i += 1;
    }
    endsWithBar = false;
  }
  cells.push(cell.trim());
  if (text.startsWith('|')) cells.shift();
  if (endsWithBar) cells.pop();
  return { cells, separated };
}

/** 分隔行定的对齐，一列一个；不是分隔行的是 `null`。 */
function alignments(line) {
  const { cells, separated } = row(line);
  if (!separated || !cells.length) return null;
  const out = [];
  for (const cell of cells) {
    const marker = /^(:)?-{3,}(:)?$/.exec(cell);
    if (!marker) return null;
    out.push(marker[1] && marker[2] ? 'center' : marker[2] ? 'right' : marker[1] ? 'left' : '');
  }
  return out;
}

/** 这一行加下一行是不是表格的头：表头和分隔行的列数得一样。 */
function isTableStart(lines, i) {
  if (i + 1 >= lines.length) return false;
  const head = row(lines[i]);
  const align = alignments(lines[i + 1]);
  return Boolean(align && head.separated && head.cells.length === align.length);
}

/**
 * 一张表：表头、分隔行，下面一行行到空行或者没有 `|` 的一行为止；格数照表头，少的补空，多的丢掉。
 * @returns {{block: Block, next: number}} `next` 是表格后面那一行
 */
function table(lines, start) {
  const head = row(lines[start]).cells;
  const align = /** @type {string[]} */ (alignments(lines[start + 1]));
  const rows = [];
  let i = start + 2;
  for (; i < lines.length && lines[i].trim(); i += 1) {
    const r = row(lines[i]);
    if (!r.separated) break;
    rows.push(head.map((_, k) => inline(r.cells[k] ?? '')));
  }
  return { block: { type: 'table', align, head: head.map((c) => inline(c)), rows }, next: i };
}

/**
 * 从这一行起的列表：先认无序的、再认有序的；连着的项合成一张，缩进的也摊平（不嵌套）；一行不是项就结束。
 * 无序的项以 `[ ]`、`[x]` 开头的是任务，有一项是任务整张就是任务列表。不是列表的是 `null`。
 * @returns {{block: Block, next: number}|null}
 */
function list(lines, start) {
  for (const [pattern, ordered] of /** @type {[RegExp, boolean][]} */ ([[BULLET, false], [NUMBERED, true]])) {
    if (!pattern.test(lines[start])) continue;
    const items = [];
    let i = start;
    for (let m; i < lines.length && (m = pattern.exec(lines[i])); i += 1) {
      const task = ordered ? null : TASK.exec(m[1]);
      items.push(task ? { check: task[1] !== ' ', kids: inline(task[2]) } : { check: null, kids: inline(m[1]) });
    }
    return { block: { type: 'list', ordered, task: items.some((it) => it.check != null), items }, next: i };
  }
  return null;
}
