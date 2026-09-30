// @ts-check
//! 选一样的浮层（蓝图 `web.md`「界面语言」第 3 条，照 `tui.md`「界面语言」）：浮在输入框上面，和命令列表同一个位置、同一个样子，
//! 进出照「动效」。头一行标题、右边暗色写按键；下面一行一样，现在的那一行右边暗色写「当前」，打开时选中它。
//!
//! `↑` `↓` 选（到头绕回去）、`Enter` 选定、`Esc` 关；鼠标悬停选中，点一下选定；点浮层外面关。开着时这几个键在整页最先接走
//! （焦点还在输入框里，不然 `Enter` 会把框里的字发出去）。

import { h, replace } from './dom.js';
import { show, hide } from '../lib/motion.js';

/**
 * @typedef {{title: string, hint: string, note: string, items: {label: string}[], current: number,
 *   choose: (index: number) => void}} Ask 标题、按键的提示、「当前」怎么写、一行一样、现在是哪一行、选定了做什么
 */

export class Picker {
  constructor() {
    this.title = h('strong.picker-title');
    this.hint = h('span.picker-hint');
    this.list = h('div.picker-list', { role: 'listbox' });
    this.el = h('div.dock-picker.dock-float', { hidden: true, role: 'dialog' }, h('div.picker-head', this.title, this.hint), this.list);
    this.ask = /** @type {Ask|null} */ (null);
    this.selected = 0;
    /** 上一次鼠标在哪：列表在鼠标底下动时浏览器也发 `mousemove`，位置没变的不算悬停 */
    this.pointer = '';
    this.onKey = (/** @type {KeyboardEvent} */ e) => {
      const n = this.ask?.items.length ?? 0;
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') this.mark((this.selected + (e.key === 'ArrowDown' ? 1 : n - 1)) % n);
      else if (e.key === 'Enter' && !e.isComposing) this.pick(this.selected);
      else if (e.key === 'Escape') this.close();
      else return;
      e.preventDefault();
      e.stopPropagation();
    };
    this.onDown = (/** @type {PointerEvent} */ e) => {
      if (!this.el.contains(/** @type {Node} */ (e.target))) this.close();
    };
  }

  get open() { return !!this.ask; }

  /** @param {Ask} ask */
  show(ask) {
    this.ask = ask;
    this.title.textContent = ask.title;
    this.hint.textContent = ask.hint;
    replace(this.list, ask.items.map((item, i) => h('div.picker-row', {
      role: 'option',
      // 焦点留在输入框里
      onmousedown: (/** @type {MouseEvent} */ e) => e.preventDefault(),
      onmousemove: (/** @type {MouseEvent} */ e) => this.hover(e, i),
      onclick: () => this.pick(i),
    }, h('span.picker-label', item.label), i === ask.current ? h('span.picker-note', ask.note) : null)));
    this.mark(ask.current);
    show(this.el);
    document.addEventListener('keydown', this.onKey, true);
    document.addEventListener('pointerdown', this.onDown, true);
  }

  close() {
    if (!this.ask) return;
    this.ask = null;
    document.removeEventListener('keydown', this.onKey, true);
    document.removeEventListener('pointerdown', this.onDown, true);
    hide(this.el);
  }

  /** 选定一行：先关掉，再照它做。 */
  pick(i) {
    const ask = this.ask;
    this.close();
    ask?.choose(i);
  }

  /** 选中第 `i` 行。 */
  mark(i) {
    this.selected = i;
    [...this.list.children].forEach((row, j) => {
      row.classList.toggle('is-selected', j === i);
      row.setAttribute('aria-selected', String(j === i));
    });
  }

  hover(e, i) {
    const at = `${e.clientX},${e.clientY}`;
    const moved = at !== this.pointer;
    this.pointer = at;
    if (moved && i !== this.selected) this.mark(i);
  }
}
