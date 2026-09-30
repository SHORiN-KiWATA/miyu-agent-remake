// @ts-check
//! 输入历史列表（蓝图 `web.md`「输入历史」第 4 条，照 `tui.md`「输入历史列表」）：在输入框里按 `Ctrl+R` 开，浮在输入框上面，和命令
//! 列表同一个位置、同一个样子（带 `dock-float`，吉祥物照它站上去）。头一行「历史 · N 条」和搜索框（打的字进这里，输入法照常能用），
//! 最下面一行暗色写按键。最新的贴着底、越早越往上，最多露 `history_rows` 行；打开时选中最新的一条。
//!
//! `↑`、再按 `Ctrl+R` 往更早的走，`↓` 往更新的走，到头就停；`Tab` 展开、收回选中的那一条；`Enter` 填进输入框；`Esc` 关。鼠标悬停
//! 选中，点一下等于 `Enter`；点外面关。哪一条露出来、怎么切段是 `model/history.js` 算的，这里只画。

import { h, replace } from './dom.js';
import { show, hide } from '../lib/motion.js';
import { res, t } from '../util/res.js';
import { search, firstLine, commandHead, pieces } from '../model/history.js';
import { ago } from '../model/ago.js';

/** @typedef {import('../model/history.js').Item} Item */

export class HistoryList {
  /**
   * @param {{choose: (item: Item) => void, closed: () => void}} on 选定了一条（填进输入框，带的附件跟着回来）；关掉了（焦点回输入框）
   */
  constructor(on) {
    this.on = on;
    this.count = h('span.history-count');
    this.query = /** @type {HTMLInputElement} */ (h('input.history-search', {
      // 普通的文字框：搜索框（type=search）浏览器自带一个清除的叉，样子不搭
      type: 'text',
      placeholder: t('history.search'),
      oninput: () => this.filter(),
      onkeydown: (/** @type {KeyboardEvent} */ e) => this.key(e),
    }));
    this.list = h('div.history-list', { role: 'listbox', style: `--rows: ${res.layout.history_rows}` });
    this.el = h('div.dock-history.dock-float', { hidden: true, role: 'dialog' },
      h('div.history-head', h('strong.history-title', t('history.title')), this.count, this.query),
      this.list,
      h('div.history-hint', t('history.hint')));
    /** @type {Item[]} 从新到旧 */
    this.items = [];
    /** @type {ReturnType<typeof search>} 搜出来的，从新到旧 */
    this.hits = [];
    /** 选中搜出来的第几条（0 是最新的） */
    this.selected = 0;
    /** 展开成全文的（按整条历史里的位置记，关掉再开回到一行） */
    this.expanded = new Set();
    this.pointer = '';
    this.onDown = (/** @type {PointerEvent} */ e) => {
      if (!this.el.contains(/** @type {Node} */ (e.target))) this.close();
    };
  }

  get open() { return !this.el.hidden && !this.el.classList.contains('is-leaving'); }

  /** @param {Item[]} items 从新到旧 */
  show(items) {
    this.items = items;
    this.expanded.clear();
    this.query.value = '';
    this.filter();
    show(this.el);
    this.query.focus();
    document.addEventListener('pointerdown', this.onDown, true);
  }

  close() {
    if (!this.open) return;
    document.removeEventListener('pointerdown', this.onDown, true);
    hide(this.el);
    this.on.closed();
  }

  /** 搜索框里的字变了：重新搜，选中回到最新的一条。 */
  filter() {
    this.hits = search(this.items, this.query.value.trim());
    this.selected = 0;
    this.count.textContent = t('history.count', { count: this.hits.length });
    this.draw();
  }

  draw() {
    const now = Date.now();
    // 最新的贴着底：从旧往新排
    const rows = this.hits.map((hit, i) => this.row(hit, i, now)).reverse();
    replace(this.list, rows.length ? rows : [h('div.history-empty', t('history.no_match'))]);
    this.mark();
  }

  /** 一条一行：第一行（展开的是全文，最多几行）、后面还有几行、多久以前。 */
  row(hit, i, now) {
    const cmd = commandHead(hit.text);
    const open = this.expanded.has(hit.index);
    const { line, more } = firstLine(hit.text);
    const max = res.layout.history_preview_rows;
    const lines = hit.text.split('\n');
    const shown = open ? (lines.length > max ? lines.slice(0, max - 1).join('\n') : hit.text) : line;
    const text = h('span.history-text', pieces(shown, hit.marks, cmd).map((p) => h(`span${p.cmd ? '.is-cmd' : ''}${p.mark ? '.is-mark' : ''}`, p.text)));
    const tail = open
      ? (lines.length > max ? h('span.history-rest', t('history.more', { count: lines.length - (max - 1) })) : null)
      : (more ? h('span.history-lines', t('history.lines', { count: more })) : null);
    // 带的附件：「 · N 个附件」，只有附件没有字的写「N 个附件」
    const files = Object.values(hit.parts ?? {}).reduce((n, v) => n + (Array.isArray(v) ? v.length : 0), 0);
    const attached = files ? h('span.history-lines', t(hit.text ? 'history.attachments' : 'history.attachments_only', { count: files })) : null;
    return h(`div.history-row${open ? '.is-open' : ''}`, {
      role: 'option',
      onmousedown: (/** @type {MouseEvent} */ e) => e.preventDefault(),
      onmousemove: (/** @type {MouseEvent} */ e) => this.hover(e, i),
      onclick: () => this.pick(i),
    }, h('span.history-body', text, tail, attached), h('span.history-ago', ago(hit.at, now)));
  }

  /** 画选中的那一条，滚进视野（只滚列表自己）。 */
  mark() {
    const rows = /** @type {HTMLElement[]} */ ([...this.list.querySelectorAll('.history-row')]);
    // 列表是从旧往新排的：选中的第几条（从新数）在倒数第几行
    const at = rows.length - 1 - this.selected;
    rows.forEach((r, j) => {
      r.classList.toggle('is-selected', j === at);
      r.setAttribute('aria-selected', String(j === at));
    });
    const row = rows[at];
    if (!row) return;
    const top = row.offsetTop - this.list.offsetTop;
    const bottom = top + row.offsetHeight;
    if (top < this.list.scrollTop) this.list.scrollTop = top;
    else if (bottom > this.list.scrollTop + this.list.clientHeight) this.list.scrollTop = bottom - this.list.clientHeight;
  }

  hover(e, i) {
    const at = `${e.clientX},${e.clientY}`;
    const moved = at !== this.pointer;
    this.pointer = at;
    if (moved && i !== this.selected) {
      this.selected = i;
      this.mark();
    }
  }

  /** 选定：关掉，填进输入框。 */
  pick(i) {
    const hit = this.hits[i];
    if (!hit) return;
    this.close();
    this.on.choose(hit);
  }

  /** 搜索框里的按键：`↑`、`Ctrl+R` 更早，`↓` 更新，`Tab` 展开收回，`Enter` 选定，`Esc` 关；别的照常打进搜索框。 */
  key(e) {
    if (e.isComposing) return;
    const last = this.hits.length - 1;
    if (e.key === 'ArrowUp' || (e.key.toLowerCase() === 'r' && e.ctrlKey && !e.altKey && !e.metaKey)) this.move(Math.min(last, this.selected + 1));
    else if (e.key === 'ArrowDown') this.move(Math.max(0, this.selected - 1));
    else if (e.key === 'Tab' && !e.shiftKey) this.toggle();
    else if (e.key === 'Enter') this.pick(this.selected);
    else if (e.key === 'Escape') this.close();
    else return;
    e.preventDefault();
    e.stopPropagation();
  }

  move(i) {
    if (i < 0 || i === this.selected) return;
    this.selected = i;
    this.mark();
  }

  /** 选中的那一条展开成全文、收回一行（展开是那一条自己的，可以同时展开好几条）。 */
  toggle() {
    const hit = this.hits[this.selected];
    if (!hit) return;
    if (this.expanded.has(hit.index)) this.expanded.delete(hit.index);
    else this.expanded.add(hit.index);
    this.draw();
  }
}
