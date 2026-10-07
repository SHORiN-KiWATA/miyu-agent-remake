// @ts-check
//! 按钮旁边弹出的小菜单（蓝图 `web.md`「人格、预设、工作区」第 3、4 条）：顶上一行标题、右边暗色写按键（照选语言的浮层），下面一行一项
//! （名字，有说明的下面暗色一行；现在的那一项打勾，用不了的暗着），可以分几段（段名暗色），再下面可以有一个输入框（回车交）、一排按钮。
//! 宽度跟着字走。`↑` `↓` 选、`Enter` 选定、`Esc` 和点外面关；鼠标悬停选中、点一下选定；`Backspace` 交给 `back`（文件夹里回上一级）。
//! 不画描边（浮起来的层那一圈细线除外）。

import { h, icon, replace } from '../../src/lib/dom.js';

/**
 * @typedef {{title: string, desc?: string, icon?: string, current?: boolean, off?: boolean, tip?: string, pick: () => void}} Row
 * @typedef {{section: string}} Section
 * @typedef {{hint: string, submit: (text: string) => Promise<string|null>}} Input 回车交；交回一句错就写在框下面，`null` 是成了
 * @typedef {{title: string, hint?: string, rows: (Row|Section)[], note?: string, input?: Input, foot?: HTMLElement[], back?: () => void, below?: boolean}} View
 */

export class Menu {
  constructor() {
    this.el = h('div.setup-menu', { hidden: true, role: 'menu' });
    /** @type {{el: HTMLElement, row: Row}[]} */
    this.items = [];
    this.at = -1;
    /** @type {View|null} */
    this.view = null;
    this.onKey = (/** @type {KeyboardEvent} */ e) => {
      const inField = /** @type {HTMLElement} */ (e.target).classList?.contains('setup-input');
      if (e.key === 'Escape') this.close();
      else if (inField) return;
      else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') this.mark(this.next(e.key === 'ArrowDown' ? 1 : -1));
      else if (e.key === 'Enter' && !e.isComposing && this.at >= 0) this.pick(this.items[this.at].row);
      else if (e.key === 'Backspace' && this.view?.back) this.view.back();
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

  /** 在 `anchor` 旁边打开一屏（已经开着的换成这一屏）：`below` 开在它下面（左上角那一条），不然开在上面。 @param {HTMLElement} anchor @param {View} view */
  show(anchor, view) {
    this.anchor = anchor;
    this.view = view;
    this.items = [];
    const nodes = view.rows.map((r) => {
      if ('section' in r) return h('div.setup-menu-section', r.section);
      const el = h(`div.setup-menu-row${r.current ? '.is-current' : ''}${r.off ? '.is-off' : ''}`, {
        role: 'menuitem', title: r.tip ?? null,
        onmousedown: (/** @type {MouseEvent} */ e) => e.preventDefault(),
        onmousemove: () => { if (!r.off) this.mark(this.items.findIndex((x) => x.row === r)); },
        onclick: () => this.pick(r),
      }, r.icon ? icon(r.icon) : null, h('div.setup-menu-text', h('span.setup-menu-title', r.title), r.desc ? h('span.setup-menu-desc', r.desc) : null), icon('check'));
      this.items.push({ el, row: r });
      return el;
    });
    replace(this.el,
      h('div.setup-menu-head', h('strong.setup-menu-name', view.title), view.hint ? h('span.setup-menu-hint', view.hint) : null),
      h('div.setup-menu-list', nodes, view.note ? h('div.setup-menu-note', view.note) : null),
      view.input ? this.inputBox(view.input) : null,
      view.foot?.length ? h('div.setup-menu-foot', view.foot) : null);
    this.el.classList.toggle('is-below', !!view.below);
    this.el.style.left = `${anchor.offsetLeft}px`;
    const wasOpen = !this.el.hidden;
    this.el.hidden = false;
    this.fit();
    this.mark(Math.max(0, this.items.findIndex((x) => x.row.current && !x.row.off)));
    if (!wasOpen) {
      document.addEventListener('keydown', this.onKey, true);
      document.addEventListener('pointerdown', this.onDown, true);
    }
  }

  /** 放不下（空会话里输入框在窗口中间，上面地方少）：列表那一截矮下去，整个菜单留在窗口里。 */
  fit() {
    const list = /** @type {HTMLElement|null} */ (this.el.querySelector('.setup-menu-list'));
    if (!list) return;
    list.style.maxHeight = '';
    const r = this.el.getBoundingClientRect();
    const over = this.view?.below ? r.bottom - (innerHeight - 8) : 8 - r.top;
    if (over > 0) list.style.maxHeight = `${Math.max(96, list.clientHeight - over)}px`;
  }

  close() {
    if (this.el.hidden) return;
    this.el.hidden = true;
    this.view = null;
    document.removeEventListener('keydown', this.onKey, true);
    document.removeEventListener('pointerdown', this.onDown, true);
  }

  /** 输入框：回车交，错了写在下面、字留着。 @param {Input} input */
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
      const i = (((this.at + step * k) % n) + n) % n;
      if (!this.items[i].row.off) return i;
    }
    return this.at;
  }

  /** @param {number} i */
  mark(i) {
    this.at = i;
    this.items.forEach((x, j) => x.el.classList.toggle('is-selected', j === i));
    this.items[i]?.el.scrollIntoView({ block: 'nearest' });
  }

  /** 选定一项：先关上（它要换一屏的自己再开）。 @param {Row} r */
  pick(r) {
    if (r.off) return;
    this.close();
    r.pick();
  }
}
