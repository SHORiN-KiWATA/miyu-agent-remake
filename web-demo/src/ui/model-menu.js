// @ts-check
//! 换模型的菜单（蓝图 `web.md`「换模型的菜单」，照 Claude 网页端的模型菜单）：从框下面那一行的模型那一截往上弹。一个列表（最多露
//! 几行，多的在里面滚）；配了池的最下面一个「模型 | 模型池」切换，列表左右滑着换页。选了不关，勾挪过去。列什么由
//! `model/model-menu.js` 排。
//!
//! 开着时焦点还在输入框里：`↑` `↓` `Enter` `Tab` `Esc` 在整页最先接走（照选语言的浮层 `picker.js`）。进出照「动效」。

import { h, icon, replace, scaleOf } from './dom.js';
import { show, hide } from '../lib/motion.js';
import { res, t } from '../util/res.js';
import { menuOf } from '../model/model-menu.js';

/**
 * @typedef {import('../model/model-menu.js').Row} Row
 * @typedef {{load: () => Promise<any>, current: () => string|null, choose: (row: Row) => void}} On 问核心要列表；会话现在的引用
 *   （选过还没生效的照选的）；选定了一行
 */

export class ModelMenu {
  /** @param {On} on */
  constructor(on) {
    this.on = on;
    this.list = h('div.model-menu-list', { role: 'menu', style: `--rows: ${res.layout.model_menu_rows}` });
    this.pages = h('div.model-menu-pages', this.list);
    this.switchEl = h('div.model-menu-switch', { hidden: true });
    this.el = h('div.model-menu', { hidden: true, style: `--menu-w: ${res.layout.model_menu_width}px; --menu-min: ${res.layout.model_menu_min_width}px` }, this.pages, this.switchEl);
    this.isOpen = false;
    /** 列表：`model.list` 的回应；在哪一页；能选的几行、选中第几行 */
    this.data = /** @type {any} */ (null);
    this.page = /** @type {'models'|'pools'} */ ('models');
    this.rows = /** @type {{el: HTMLElement, row: Row}[]} */ ([]);
    this.at = 0;
    this.seq = 0;
    this.onKey = (/** @type {KeyboardEvent} */ e) => this.key(e);
    this.onDown = (/** @type {PointerEvent} */ e) => {
      const target = /** @type {Node} */ (e.target);
      if (!this.el.contains(target) && !this.anchor?.contains(target)) this.close();
    };
    /** 鼠标移走自动收起（蓝图「换模型的菜单」第 3 条）：进过菜单或那一截以后，离开这两处一会儿收起，回来的不收 */
    this.leaveTimer = 0;
    this.hovered = false;
    const enter = (/** @type {PointerEvent} */ e) => {
      if (e.pointerType !== 'mouse') return;
      this.hovered = true;
      clearTimeout(this.leaveTimer);
    };
    const leave = (/** @type {PointerEvent} */ e) => {
      if (e.pointerType !== 'mouse' || !this.isOpen || !this.hovered) return;
      clearTimeout(this.leaveTimer);
      this.leaveTimer = window.setTimeout(() => this.close(), res.layout.model_menu_leave_ms);
    };
    this.el.addEventListener('pointerenter', enter);
    this.el.addEventListener('pointerleave', leave);
    this.hover = { enter, leave };
  }

  /** @param {HTMLElement} anchor 模型那一截：开着再点是关 */
  toggle(anchor) {
    if (this.isOpen) this.close();
    else this.open(anchor);
  }

  /** @param {HTMLElement} anchor */
  async open(anchor) {
    if (this.anchor !== anchor) {
      this.anchor?.removeEventListener('pointerenter', this.hover.enter);
      this.anchor?.removeEventListener('pointerleave', this.hover.leave);
      anchor.addEventListener('pointerenter', this.hover.enter);
      anchor.addEventListener('pointerleave', this.hover.leave);
    }
    this.anchor = anchor;
    // 量到的是屏幕上的像素，整页放大（`--ui-scale`）以后要换回菜单自己的 CSS 像素（`scaleOf`）
    const parent = this.el.parentElement;
    const scale = parent ? scaleOf(parent) : 1;
    const box = anchor.getBoundingClientRect();
    this.el.style.left = `${Math.max(0, (box.left - (parent?.getBoundingClientRect().left ?? 0)) / scale)}px`;
    // 从那一截长出来：放大的原点在它的中点（蓝图「换模型的菜单」第 2 条）
    this.el.style.setProperty('--origin', `${box.width / scale / 2}px`);
    // 鼠标在那一截上点开的算进来过；键盘、`/model` 开的没有
    this.hovered = anchor.matches(':hover');
    clearTimeout(this.leaveTimer);
    this.isOpen = true;
    anchor.classList.add('is-open');
    this.data = null;
    this.switchEl.hidden = true;
    replace(this.list, h('div.model-menu-note', t('model_menu.loading')));
    this.rows = [];
    show(this.el);
    document.addEventListener('keydown', this.onKey, true);
    document.addEventListener('pointerdown', this.onDown, true);
    const seq = ++this.seq;
    const got = await this.on.load().catch((err) => ({ error: err?.message ?? String(err) }));
    if (seq !== this.seq || !this.isOpen) return;
    if (got?.error) {
      replace(this.list, h('div.model-menu-note', t('model_menu.failed', { message: got.error })));
      return;
    }
    this.data = got;
    // 用着池的直接在模型池那一页
    this.page = (this.on.current() ?? '').startsWith('@') && got.pools?.length ? 'pools' : 'models';
    // 宽照模型那一页最长的名字定下来（最窄、最宽见 CSS），换页不变：不然切到模型池时菜单左右跳
    this.el.style.width = '';
    const page = this.page;
    this.page = 'models';
    this.draw();
    this.el.style.width = `${this.el.offsetWidth}px`;
    this.page = page;
    this.draw();
    this.mark(Math.max(0, this.rows.findIndex((r) => r.row.current)), true);
  }

  close() {
    if (!this.isOpen) return;
    this.isOpen = false;
    clearTimeout(this.leaveTimer);
    this.seq += 1;
    this.anchor?.classList.remove('is-open');
    document.removeEventListener('keydown', this.onKey, true);
    document.removeEventListener('pointerdown', this.onDown, true);
    hide(this.el);
  }

  /** 画这一页（选了一行以后原地重画：不关、不重放进场）。`slide` 是换页时列表从哪边滑进来。 */
  draw(slide = '') {
    const menu = menuOf(this.data, this.on.current());
    const rows = this.page === 'pools' ? menu.pools : menu.models;
    this.rows = rows.map((row) => ({ row, el: this.rowEl(row) }));
    this.list.className = `model-menu-list${slide ? ` ${slide}` : ''}`;
    replace(this.list, this.rows.length ? this.rows.map((r) => r.el) : h('div.model-menu-note', t('model_menu.empty')));
    // 配了池的：最下面「模型 | 模型池」切换
    this.switchEl.hidden = !menu.pools.length;
    if (menu.pools.length) {
      this.switchEl.classList.toggle('on-pools', this.page === 'pools');
      replace(this.switchEl, [h('span.model-menu-thumb'), ...(/** @type {const} */ (['models', 'pools'])).map((page) => h('button', {
        type: 'button', 'aria-pressed': String(this.page === page),
        onmousedown: (/** @type {MouseEvent} */ e) => e.preventDefault(),
        onclick: () => this.turn(page),
      }, t(`model_menu.${page}`)))]);
    }
  }

  /** 一行：上面名字、下面小字；现在用着的右边一个勾；用不了的暗、不能选，悬停写原因。 */
  rowEl(row) {
    const el = h(`div.model-menu-row${row.usable ? '' : '.is-off'}`, {
      role: 'menuitem', 'aria-disabled': String(!row.usable), title: row.why || null,
      onmousedown: (/** @type {MouseEvent} */ e) => e.preventDefault(),
      onmousemove: () => {
        const i = this.rows.findIndex((r) => r.row === row);
        if (i >= 0 && i !== this.at) this.mark(i, false);
      },
      onclick: () => this.choose(row),
    }, h('span.model-menu-text', h('span.model-menu-title', row.title), h('span.model-menu-desc', row.desc)), icon('check'));
    el.classList.toggle('is-current', row.current);
    return el;
  }

  /** 换页：列表从一边滑进来，高度从原来的缓到新的。 */
  turn(page) {
    if (this.page === page || !this.data) return;
    const from = this.pages.offsetHeight;
    this.page = page;
    this.draw(page === 'pools' ? 'is-from-right' : 'is-from-left');
    const to = this.pages.offsetHeight;
    this.pages.style.height = `${from}px`;
    requestAnimationFrame(() => {
      this.pages.style.height = `${to}px`;
      this.pages.addEventListener('transitionend', () => { this.pages.style.height = ''; }, { once: true });
    });
    this.mark(Math.max(0, this.rows.findIndex((r) => r.row.current)), true);
  }

  /** 选中第 `i` 行；`scroll` 是键盘选的：滚进视野（鼠标悬停的不滚，免得往上滑时列表跟着滚）。 */
  mark(i, scroll) {
    this.at = i;
    this.rows.forEach((r, j) => r.el.classList.toggle('is-selected', i === j));
    if (scroll) this.rows[i]?.el.scrollIntoView({ block: 'nearest' });
  }

  /** 选定：交出去，菜单不关，照新的现在原地重画（勾挪过去）。 */
  choose(row) {
    if (!row.usable || row.current) return;
    this.on.choose(row);
    const at = this.at;
    this.draw();
    this.mark(at, false);
  }

  /** @param {KeyboardEvent} e */
  key(e) {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      const step = e.key === 'ArrowDown' ? 1 : -1;
      let i = this.at + step;
      while (this.rows[i] && !this.rows[i].row.usable) i += step;
      if (this.rows[i]) this.mark(i, true);
    } else if (e.key === 'Enter' && !e.isComposing) {
      const row = this.rows[this.at]?.row;
      if (row) this.choose(row);
    } else if (e.key === 'Tab' && !this.switchEl.hidden) this.turn(this.page === 'models' ? 'pools' : 'models');
    else if (e.key === 'Escape') this.close();
    else return;
    e.preventDefault();
    e.stopPropagation();
  }
}
