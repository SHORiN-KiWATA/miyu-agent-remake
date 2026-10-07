// @ts-check
//! 按钮旁边弹出的小菜单（蓝图 `web.md`「人格、预设、工作区」第 3、4 条）：一行一项（名字、暗色的一行说明，现在的那一项打勾，
//! 用不了的暗着），可以分几段（段名暗色），最下面可以有一个输入框（回车交）。`↑` `↓` 选、`Enter` 选定、`Esc` 和点外面关；
//! 鼠标悬停选中、点一下选定。样子和换模型的菜单一样，不画描边（浮起来的层那一圈细线除外）。

import { h, icon, replace } from '../../src/lib/dom.js';

/**
 * @typedef {{title: string, desc?: string, current?: boolean, off?: boolean, tip?: string, pick: () => void}} Row
 * @typedef {{section: string}} Section
 * @typedef {{hint: string, submit: (text: string) => Promise<string|null>}} Input 回车交；交回一句错就写在框下面，`null` 是成了
 */

export class Menu {
  constructor() {
    this.el = h('div.setup-menu', { hidden: true, role: 'menu' });
    /** @type {{el: HTMLElement, row: Row}[]} */
    this.items = [];
    this.at = -1;
    this.onKey = (/** @type {KeyboardEvent} */ e) => {
      if (e.key === 'Escape') this.close();
      else if (/** @type {HTMLElement} */ (e.target).classList?.contains('setup-input')) return;
      else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') this.mark(this.next(e.key === 'ArrowDown' ? 1 : -1));
      else if (e.key === 'Enter' && !e.isComposing && this.at >= 0) this.pick(this.items[this.at].row);
      else return;
      e.preventDefault();
      e.stopPropagation();
    };
    this.onDown = (/** @type {PointerEvent} */ e) => {
      const target = /** @type {Node} */ (e.target);
      if (!this.el.contains(target) && !this.anchor?.contains(target)) this.close();
    };
  }

  get open() { return !this.el.hidden; }

  /**
   * 在 `anchor` 旁边打开：`below` 开在它下面（左上角那一条），不然开在上面（输入框上面那一排）。
   * @param {HTMLElement} anchor @param {(Row|Section)[]} rows @param {Input|null} [input] @param {boolean} [below]
   */
  show(anchor, rows, input = null, below = false) {
    this.anchor = anchor;
    this.items = [];
    const nodes = rows.map((r) => {
      if ('section' in r) return h('div.setup-menu-section', r.section);
      const el = h(`div.setup-menu-row${r.current ? '.is-current' : ''}${r.off ? '.is-off' : ''}`, {
        role: 'menuitem', title: r.tip ?? null,
        onmousedown: (/** @type {MouseEvent} */ e) => e.preventDefault(),
        onmousemove: () => { if (!r.off) this.mark(this.items.findIndex((x) => x.row === r)); },
        onclick: () => this.pick(r),
      }, h('div.setup-menu-text', h('span.setup-menu-title', r.title), r.desc ? h('span.setup-menu-desc', r.desc) : null), icon('check'));
      this.items.push({ el, row: r });
      return el;
    });
    const box = input ? this.inputBox(input) : null;
    replace(this.el, nodes, box);
    this.el.classList.toggle('is-below', below);
    this.el.style.left = `${anchor.offsetLeft}px`;
    this.el.hidden = false;
    this.mark(Math.max(0, this.items.findIndex((x) => x.row.current)));
    document.addEventListener('keydown', this.onKey, true);
    document.addEventListener('pointerdown', this.onDown, true);
  }

  close() {
    if (this.el.hidden) return;
    this.el.hidden = true;
    document.removeEventListener('keydown', this.onKey, true);
    document.removeEventListener('pointerdown', this.onDown, true);
  }

  /** 最下面的输入框：回车交，错了写在下面、字留着。 @param {Input} input */
  inputBox(input) {
    const err = h('div.setup-menu-error', { hidden: true });
    const field = /** @type {HTMLInputElement} */ (h('input.setup-input', { type: 'text', placeholder: input.hint, spellcheck: 'false', autocomplete: 'off' }));
    field.addEventListener('keydown', async (e) => {
      if (e.key !== 'Enter' || e.isComposing) return;
      e.preventDefault();
      const why = await input.submit(field.value);
      err.hidden = !why;
      err.textContent = why ?? '';
      if (!why) this.close();
    });
    return h('div.setup-menu-input', field, err);
  }

  /** 下一个能选的（跳过暗着的），到头绕回去。 @param {number} step */
  next(step) {
    const n = this.items.length;
    for (let k = 1; k <= n; k++) {
      const i = (this.at + step * k + n * k) % n;
      if (!this.items[i].row.off) return i;
    }
    return this.at;
  }

  /** @param {number} i */
  mark(i) {
    this.at = i;
    this.items.forEach((x, j) => x.el.classList.toggle('is-selected', j === i));
  }

  /** @param {Row} r */
  pick(r) {
    if (r.off) return;
    this.close();
    r.pick();
  }
}
