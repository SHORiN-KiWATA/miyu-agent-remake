// @ts-check
//! 输入历史（蓝图 `web.md`「输入历史」，照 `tui.md`「按键」的 `↑` `↓`、「输入历史列表」）：记什么、`↑` `↓` 怎么翻、两下 `Esc`
//! 清掉的那句怎么拿回来、列表怎么搜、一条怎么写成一行。纯的：框里的字、光标在哪都从外面给。

/**
 * @typedef {{text: string, at: number, session?: string, parts?: Record<string, any>}} Item 一条：原样的字、发出去的时刻；带了附件的
 *   还有发在哪个会话、跟着话一起发的几样记成的样子（按挂载位 `composer.payload` 里那一件的编号，附件是核心存好的那一份）
 * @typedef {{session: string, parts: Record<string, any>}} Extra 带过去的附件那些
 */
/** @typedef {{start: boolean, first: boolean, last: boolean}} Caret 光标：在最前面（没选着字）、前面没有换行、后面没有换行 */

/**
 * 记一条：放在最前面（从新到旧）；没字也没附件的不记，和最新的一条一模一样（字一样、都没带附件）的不再记一遍，最多 `max` 条。
 * 交回新的一份，不改原来的。
 * @param {Item[]} items
 * @param {string} text
 * @param {number} at
 * @param {number} max
 * @param {Extra|null} [extra] 带过去的附件那些
 */
export function remember(items, text, at, max, extra = null) {
  if (!extra && (text.trim() === '' || (items[0]?.text === text && !items[0]?.parts))) return items;
  return [{ text, at, ...(extra ?? {}) }, ...items].slice(0, max);
}

/**
 * 翻输入历史的状态。`↑` `↓` 交回要放进框里的字；不接的交回 `null`（归浏览器挪光标）。
 *
 * - 没在翻：框里空着、或者光标在最前面按 `↑`，翻出最新的一条（先拿回两下 `Esc` 清掉的那句，见 `clear`）；框里那句没发的记着。
 * - 翻着（框里还是翻出来的那一条，没改过）：光标前面没有换行按 `↑` 往更早的走，到头就停；后面没有换行按 `↓` 往更新的走，
 *   走过最新的一条还给那句没发的。
 * - 翻出来的改了就不算在翻，改过的那句当成没发的。
 */
export class Recall {
  /** @param {Item[]} items 从新到旧 */
  constructor(items) {
    this.items = items;
    /** 翻到第几条（`-1` 是没翻进历史：空着，或者拿回了清掉的那句） */
    this.at = -1;
    /** 最后放进框里的（框里还是它就是没改过）；没在翻是 `null` */
    this.shown = /** @type {string|null} */ (null);
    /** 开始翻之前框里那句没发的 */
    this.draft = '';
    /** 两下 `Esc` 清掉的那句（单独一份，不进历史） */
    this.cleared = /** @type {string|null} */ (null);
  }

  /** 框里还是放进去的那句（没改过）。 */
  fresh(value) {
    return this.shown != null && value === this.shown;
  }

  /** @param {string} value @param {Caret} caret */
  older(value, caret) {
    const fresh = this.fresh(value);
    if (!fresh) this.reset();
    if (fresh ? !caret.first : !(value === '' || caret.start)) return null;
    if (!fresh && value === '' && this.cleared != null) {
      this.shown = this.cleared;
      this.cleared = null;
      return this.shown;
    }
    if (this.at + 1 >= this.items.length) return this.at < 0 ? null : value;
    if (this.at < 0) this.draft = value;
    this.at += 1;
    this.shown = this.items[this.at].text;
    return this.shown;
  }

  /** @param {string} value @param {Caret} caret */
  newer(value, caret) {
    if (!this.fresh(value) || this.at < 0 || !caret.last) return null;
    this.at -= 1;
    if (this.at >= 0) {
      this.shown = this.items[this.at].text;
      return this.shown;
    }
    const draft = this.draft;
    this.reset();
    return draft;
  }

  /** 不在翻了（发出去了、改过了）。 */
  reset() {
    this.at = -1;
    this.shown = null;
    this.draft = '';
  }

  /** 两下 `Esc` 清掉了：留下这一份（再清一次换掉），空着按 `↑` 先拿回它。 */
  clear(text) {
    this.cleared = text;
    this.reset();
  }

  /** 发出去了一句（话或命令）：记进历史（带过去的附件一起），不在翻了。 */
  record(text, at, max, extra = null) {
    this.items = remember(this.items, text, at, max, extra);
    this.reset();
  }

  /** 翻到的是哪一条（带的附件跟着它回来）；没翻进历史（空着、没发的那句、拿回的清掉的那句）是 `null`。 */
  item() {
    return this.at >= 0 ? this.items[this.at] : null;
  }
}

/**
 * 列表搜：不分大小写，留下包含这些字的，从新到旧；每条带着对得上的那几段（`[开始, 结束)`）。空的全留、不标。
 * @param {Item[]} items
 * @param {string} query
 * @returns {(Item & {index: number, marks: [number, number][]})[]}
 */
export function search(items, query) {
  const q = query.toLowerCase();
  const out = [];
  items.forEach((item, index) => {
    if (!q) {
      out.push({ ...item, index, marks: [] });
      return;
    }
    const lower = item.text.toLowerCase();
    /** @type {[number, number][]} */
    const marks = [];
    for (let i = lower.indexOf(q); i >= 0; i = lower.indexOf(q, i + q.length)) marks.push([i, i + q.length]);
    if (marks.length) out.push({ ...item, index, marks });
  });
  return out;
}

/** 一条写成一行：第一行，后面还有几行。 */
export function firstLine(text) {
  const lines = text.split('\n');
  return { line: lines[0], more: lines.length - 1 };
}

/** 命令的名字那一截多长（`/theme` 是 6）；不是命令的（路径、普通的话）是 0。照 `model/commands.js` 认命令的规矩。 */
export function commandHead(text) {
  const m = /^\/[A-Za-z][\w-]*(?=\s|$)/.exec(text);
  return m ? m[0].length : 0;
}

/**
 * 露出来的一截字切成几段：命令的名字那一截（`cmd`，强调色）、对得上的字（`mark`，强调色加下划线），两样可以叠着。`marks` 是对整条
 * 的位置，落在这一截外面的不管。
 * @param {string} text 露出来的字（一行时是第一行，展开时是全文）
 * @param {[number, number][]} marks
 * @param {number} cmd 命令的名字那一截多长
 * @returns {{text: string, cmd: boolean, mark: boolean}[]}
 */
export function pieces(text, marks, cmd) {
  const cuts = new Set([0, text.length, Math.min(cmd, text.length)]);
  for (const [a, b] of marks) {
    if (a < text.length) cuts.add(a);
    if (b < text.length) cuts.add(b);
  }
  const at = [...cuts].sort((a, b) => a - b);
  const out = [];
  for (let i = 0; i + 1 < at.length; i++) {
    const [a, b] = [at[i], at[i + 1]];
    if (a === b) continue;
    out.push({ text: text.slice(a, b), cmd: a < cmd, mark: marks.some(([x, y]) => a >= x && b <= y) });
  }
  return out;
}
