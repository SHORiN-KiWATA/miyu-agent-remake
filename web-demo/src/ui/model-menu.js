// @ts-check
//! 换模型的菜单（蓝图 `web.md`「换模型的菜单」，照 Claude 网页端的模型菜单）：从框下面那一行的模型那一截往上弹。一个列表（最多露
//! 几行，多的在里面滚）；最下面一个「模型 | 模型池」切换，列表左右滑着换页。选了不关，勾挪过去。列什么由
//! `model/model-menu.js` 排。
//!
//! 开着时焦点还在输入框里：`↑` `↓` `Enter` `Tab` `Esc` 在整页最先接走（照选语言的浮层 `picker.js`）。进出照「动效」。

import { h, icon, replace, scaleOf } from './dom.js';
import { show, hide, span, cubicBezier, parseBezier, snapToPixels } from '../lib/motion.js';
import { res, t } from '../util/res.js';
import { menuOf, filterRows, effortRows, effortLabel } from '../model/model-menu.js';
import { logoEl } from '../lib/logo.js';

/**
 * @typedef {import('../model/model-menu.js').Row} Row
 * @typedef {{load: () => Promise<any>, cached: () => any, current: () => string|null, choose: (row: Row) => void,
 *   effort: () => {model: string|null, levels: string[], current: string|null}, chooseEffort: (level: string|null) => void}} On
 *   问核心要列表；上一次问到的列表（打开时先照它画，不等）；会话现在的引用（选过还没生效的照选的）；选定了一行；现在的模型（池是 `null`）、它报的思考强度有哪几档、现在是哪一档
 *   （`null` 是默认）；选了一档
 */

export class ModelMenu {
  /** @param {On} on */
  constructor(on) {
    this.on = on;
    this.list = h('div.model-menu-list', { role: 'menu', style: `--rows: ${res.layout.model_menu_rows}` });
    this.switchEl = h('div.model-menu-switch', {
      hidden: true, role: 'button', tabindex: '-1',
      onmousedown: (/** @type {MouseEvent} */ e) => e.preventDefault(),
      onclick: () => this.turn(this.page === 'models' ? 'pools' : 'models'),
    });
    /** 思考强度那一行（模型那一页、这个模型有几档的才有）和它往右展开的子菜单（蓝图「换模型的菜单」第 2 条） */
    this.effortValue = h('span.model-menu-value');
    this.effortRow = h('div.model-menu-row.is-item', {
      role: 'menuitem', 'aria-haspopup': 'menu',
      onmousedown: (/** @type {MouseEvent} */ e) => e.preventDefault(),
      onmousemove: () => this.hoverEffort(),
      onclick: () => this.openSub(true),
    }, h('span.model-menu-title', t('model_menu.effort.title')), this.effortValue, icon('chevron-right'));
    this.effortEl = h('div.model-menu-effort', { hidden: true }, h('div.model-menu-line'), this.effortRow);
    this.sub = h('div.model-menu.model-menu-sub', { hidden: true, role: 'menu' });
    /**
     * 一页：列表和它下面的思考强度那一行（模型那一页才有）。换页时整页一起滑进来，高度连思考强度那一行一起缓：原来那一行在页外面，
     * 换页时当场没了、当场有了，菜单上沿一下跳 45px（2026-10-02 项目主人觉得切页抖，逐帧量出来的）
     */
    this.sheet = h('div.model-menu-sheet', this.list, this.effortEl);
    this.pages = h('div.model-menu-pages', this.sheet);
    /** 搜索框：这一页的项比能露出的行多才有（蓝图「换模型的菜单」第 2 条）；照显示名、模型名、供应商就地筛 */
    this.query = '';
    this.searchInput = /** @type {HTMLInputElement} */ (h('input.model-menu-search-input', {
      type: 'search', placeholder: t('model_menu.search'), spellcheck: 'false', autocomplete: 'off',
      oninput: () => {
        // 开始搜的那一下记住列表多高：搜的时候保持这个高度（行少了菜单不缩，鼠标不会一下落到外面被收起）
        if (!this.query && this.searchInput.value) this.searchHeight = this.list.offsetHeight;
        this.query = this.searchInput.value;
        this.draw();
        this.markFirst();
      },
    }));
    this.searchEl = h('label.model-menu-search', { hidden: true }, icon('search'), this.searchInput);
    this.el = h('div.model-menu', { hidden: true, style: `--menu-w: ${res.layout.model_menu_width}px; --menu-min: ${res.layout.model_menu_min_width}px` },
      this.searchEl, this.pages, this.switchEl, this.sub);
    /** 子菜单开着吗；它的几行、选中第几行；键盘在子菜单里吗 */
    this.subOpen = false;
    this.subRows = /** @type {{el: HTMLElement, level: string|null}[]} */ ([]);
    this.subAt = 0;
    this.inSub = false;
    this.hoverTimer = 0;
    this.isOpen = false;
    /** 列表：`model.list` 的回应；在哪一页；能选的几行、选中第几行 */
    this.data = /** @type {any} */ (null);
    this.page = /** @type {'models'|'pools'} */ ('models');
    this.rows = /** @type {{el: HTMLElement, row: Row}[]} */ ([]);
    this.at = 0;
    this.seq = 0;
    /** 换页时一帧一帧缓高度的那一帧（`requestAnimationFrame`） */
    this.heightFrame = 0;
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
      // 光标在搜索框里（在打字）的不自己收：点外面、`Esc` 照样关
      if (e.pointerType !== 'mouse' || !this.isOpen || !this.hovered || document.activeElement === this.searchInput) return;
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
    // 上一次换页缓到一半就关了的：钉着的高度放开（不然再开时列表被钉在模型池那一页的高度，只露一行）
    cancelAnimationFrame(this.heightFrame);
    this.pages.style.height = '';
    this.data = null;
    this.rows = [];
    this.query = '';
    this.searchInput.value = '';
    // 关了以后焦点回到原来的地方（有搜索框时打开菜单焦点进它）
    this.back = /** @type {HTMLElement|null} */ (document.activeElement);
    // 有上一次问到的列表：照它先画好再露出来（宽高一开始就是最后的样子，展开时不跳：原来先写「正在读…」又矮又窄，列表来了一下
    // 变大，2026-10-02 项目主人觉得抖）；没有的先写「正在读…」
    // 先露出来再画（藏着的量不出宽），同一帧里画完，看不到中间的样子
    show(this.el);
    const cached = this.on.cached();
    if (cached) this.fill(cached);
    else {
      this.switchEl.hidden = true;
      this.effortEl.hidden = true;
      this.el.style.width = '';
      replace(this.list, h('div.model-menu-note', t('model_menu.loading')));
    }
    document.addEventListener('keydown', this.onKey, true);
    document.addEventListener('pointerdown', this.onDown, true);
    const seq = ++this.seq;
    const got = await this.on.load().catch((err) => ({ error: err?.message ?? String(err) }));
    if (seq !== this.seq || !this.isOpen) return;
    if (got?.error) {
      if (!this.data) replace(this.list, h('div.model-menu-note', t('model_menu.failed', { message: got.error })));
      return;
    }
    // 新问到的和画着的一样就不动；不一样的照新的画（在哪一页、选中第几行照旧）
    if (this.data && JSON.stringify(got) === JSON.stringify(this.data)) return;
    if (this.data) {
      const at = this.at;
      this.data = got;
      this.draw();
      this.mark(Math.min(at, this.rows.length - 1), false);
      return;
    }
    this.fill(got);
  }

  /** 照列表画：用着池的直接在模型池那一页；宽照模型那一页最长的名字定下来（最窄、最宽见 CSS），换页不变（不然切页时左右跳）。 */
  fill(data) {
    this.data = data;
    this.page = (this.on.current() ?? '').startsWith('@') && data.pools?.length ? 'pools' : 'models';
    this.el.style.width = '';
    const page = this.page;
    this.page = 'models';
    this.draw();
    this.el.style.width = `${this.el.offsetWidth}px`;
    this.page = page;
    this.draw();
    this.mark(Math.max(0, this.rows.findIndex((r) => r.row.current)), true);
    if (!this.searchEl.hidden) this.searchInput.focus({ preventScroll: true });
  }

  /** 筛过以后选中第一个能选的。 */
  markFirst() {
    const i = this.rows.findIndex((r) => r.row.usable);
    this.mark(i >= 0 ? i : 0, true);
  }

  close() {
    if (!this.isOpen) return;
    this.isOpen = false;
    clearTimeout(this.leaveTimer);
    this.closeSub(true);
    this.seq += 1;
    this.anchor?.classList.remove('is-open');
    document.removeEventListener('keydown', this.onKey, true);
    document.removeEventListener('pointerdown', this.onDown, true);
    if (this.el.contains(document.activeElement)) this.back?.focus({ preventScroll: true });
    hide(this.el);
  }

  /** 画这一页（选了一行以后原地重画：不关、不重放进场）。`slide` 是换页时这一页从哪边滑进来。 */
  draw(slide = '') {
    const menu = menuOf(this.data, this.on.current());
    const all = this.page === 'pools' ? menu.pools : menu.models;
    // 搜索框：这一页的项比能露出的行多才有；没有的不筛
    const searchable = all.length > res.layout.model_menu_rows;
    if (!searchable) this.query = '';
    this.searchEl.hidden = !searchable;
    const rows = filterRows(all, this.query);
    this.list.style.minHeight = this.query && this.searchHeight ? `${this.searchHeight}px` : '';
    this.rows = rows.map((row) => ({ row, el: this.rowEl(row) }));
    this.sheet.className = `model-menu-sheet${slide ? ` ${slide}` : ''}`;
    const none = all.length ? t('model_menu.no_match') : t(this.page === 'pools' ? 'model_menu.no_pools' : 'model_menu.empty');
    replace(this.list, this.rows.length ? this.rows.map((r) => r.el) : h('div.model-menu-note', none));
    // 思考强度：模型那一页一直有（池不设；这个模型报了哪几档在子菜单里分）
    const effort = this.on.effort();
    this.effortEl.hidden = this.page !== 'models' || !effort.model;
    this.effortValue.textContent = effortLabel(effort.current);
    if (this.effortEl.hidden) this.closeSub(true);
    // 最下面「模型 | 模型池」切换：一直有（2026-10-02 项目主人定：前端照 demo，没配池的那一页写还没有）
    this.switchEl.hidden = false;
    this.switchEl.classList.toggle('on-pools', this.page === 'pools');
    // 整个切换点哪儿都换到另一页，不用对准那一半（2026-10-02 项目主人定）；点击接在整个切换上（构造时）
    replace(this.switchEl, [h('span.model-menu-thumb'), ...(/** @type {const} */ (['models', 'pools'])).map((page) => h('span.model-menu-tab', {
      'aria-pressed': String(this.page === page),
    }, t(`model_menu.${page}`)))]);
    // 不是换页的（打开、选了一行、新列表来了）：高度当场对齐（换页的由 slideHeight 缓过去）
    if (!slide) this.pin(this.natural());
  }

  /**
   * 列表那一块的一个 CSS 像素在屏幕上是几个像素（整页放大、屏幕缩放乘在一起）。照计算出来的宽（带小数）量，不照 `offsetWidth`：
   * 它取整了，量出来是 1.1009 不是 1.1，对齐到的不是整像素，两页的高还是差 1 像素（逐帧截图量出来的）。这一块没有内边距、边框。
   */
  pixelScale() {
    const css = Number.parseFloat(getComputedStyle(this.pages).width);
    const zoom = css > 0 ? this.pages.getBoundingClientRect().width / css : scaleOf(this.pages);
    return zoom * (window.devicePixelRatio || 1);
  }

  /** 这一页本来多高（CSS 像素，带小数）：先放开钉着的再量。 */
  natural() {
    this.pages.style.height = '';
    return (this.pages.getBoundingClientRect().height * (window.devicePixelRatio || 1)) / this.pixelScale();
  }

  /**
   * 高度钉在 `height` 对齐到整的屏幕像素的地方。平时也钉着：两页本来的高度带小数、舍法不一样，切页那一下切换那一格的字会
   * 跳 1 像素（逐帧截图量出来的）。
   * @param {number} height
   */
  pin(height) {
    this.pages.style.height = `${snapToPixels(height, this.pixelScale())}px`;
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
    }, 'logo' in row ? logoEl(row.logo, row.brand ?? '') : null, h('span.model-menu-text', h('span.model-menu-title', { title: row.title }, row.title), h('span.model-menu-desc', row.desc)), icon('check'));
    el.classList.toggle('is-current', row.current);
    return el;
  }

  /**
   * 换页：新的一页当场换上、从一边滑进来（不等：原来旧的先滑出去、新的 40% 才进来，有延迟感，2026-10-02 项目主人指出），高度从原来的
   * 缓到新的。子菜单开着的先收掉（模型池那一页没有思考强度）。
   * 高度：先放开上一次钉着的再量新的（快速来回切时量到的不是钉着的旧值），再从原来的缓到新的（`slideHeight`）。
   */
  turn(page) {
    if (this.page === page || !this.data) return;
    this.closeSub(true);
    // 换页清空搜索框
    this.query = '';
    this.searchInput.value = '';
    const from = (this.pages.getBoundingClientRect().height * (window.devicePixelRatio || 1)) / this.pixelScale();
    cancelAnimationFrame(this.heightFrame);
    this.page = page;
    this.draw(page === 'pools' ? 'is-from-right' : 'is-from-left');
    this.slideHeight(from, this.natural());
    this.mark(Math.max(0, this.rows.findIndex((r) => r.row.current)), true);
  }

  /**
   * 换页时的高度：自己一帧一帧缓（时长、曲线照 CSS 的 `--menu-turn`、`--menu-turn-ease`），每一帧对齐到整的屏幕像素。CSS 过渡缓的是
   * 小数高度，菜单从下往上长，浏览器每帧把上沿舍到整像素、有时进有时舍，整个菜单里的字跟着上下抖 1 像素（2026-10-02 项目主人觉得
   * 切页抖：放慢、逐帧截图量出来切换那一格的字离底边 33、34 像素来回跳）。缓完钉在新的高（`pin`）；减少动画的当场钉上。
   */
  slideHeight(from, to) {
    const style = getComputedStyle(this.el);
    const ms = span(style.getPropertyValue('--menu-turn') || '0s', '0s');
    const still = typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
    if (Math.abs(from - to) < 0.5 || !ms || still) {
      this.pin(to);
      return;
    }
    const ease = cubicBezier(...parseBezier(style.getPropertyValue('--menu-turn-ease')));
    const start = performance.now();
    this.pin(from);
    const step = (/** @type {number} */ now) => {
      const k = (now - start) / ms;
      if (k >= 1) {
        this.pin(to);
        return;
      }
      this.pin(from + (to - from) * ease(k));
      this.heightFrame = requestAnimationFrame(step);
    };
    this.heightFrame = requestAnimationFrame(step);
  }

  /**
   * 选中第 `i` 行（`rows.length` 是思考强度那一行）；`scroll` 是键盘选的：滚进视野（鼠标悬停的不滚，免得往上滑时列表跟着滚）。
   * 选中列表里的，收回子菜单。
   */
  mark(i, scroll) {
    this.at = i;
    this.inSub = false;
    this.rows.forEach((r, j) => r.el.classList.toggle('is-selected', i === j));
    this.effortRow.classList.toggle('is-selected', i === this.rows.length || this.subOpen);
    if (i < this.rows.length) {
      clearTimeout(this.hoverTimer);
      this.closeSub(false);
    }
    if (scroll) this.reveal(this.rows[i]?.el);
  }

  /** 键盘选中的那一行滚进列表的视野：只滚列表自己（`scrollIntoView` 会连外面能滚的一起滚，换页时高度钉着会滚错地方）。 */
  reveal(el) {
    if (!el || el.parentElement !== this.list) return;
    // 列表自己是行的 offsetParent（CSS 里 position: relative），offsetTop 照列表的内容量
    const top = el.offsetTop;
    if (top < this.list.scrollTop) this.list.scrollTop = top;
    else if (top + el.offsetHeight > this.list.scrollTop + this.list.clientHeight) this.list.scrollTop = top + el.offsetHeight - this.list.clientHeight;
  }

  /** 鼠标在思考强度那一行上：选中它，停一会儿展开子菜单。 */
  hoverEffort() {
    if (this.at === this.rows.length && !this.inSub) return;
    this.mark(this.rows.length, false);
    clearTimeout(this.hoverTimer);
    this.hoverTimer = window.setTimeout(() => this.openSub(false), res.layout.model_menu_hover_ms);
  }

  /** 展开思考强度的子菜单：贴着右沿，下沿和那一行对齐（往上长）；`select` 是键盘、点开的：选中现在那一档。 */
  openSub(select) {
    clearTimeout(this.hoverTimer);
    if (this.effortEl.hidden) return;
    if (!this.subOpen) {
      this.subOpen = true;
      this.drawSub();
      // 下沿和那一行对齐：照屏幕上的位置算，换回菜单自己的 CSS 像素（那一行在一页里，`offsetTop` 不是照菜单量的）
      const scale = scaleOf(this.el);
      const border = Number.parseFloat(getComputedStyle(this.el).borderBottomWidth) || 0;
      const gap = (this.el.getBoundingClientRect().bottom - this.effortRow.getBoundingClientRect().bottom) / scale - border;
      this.sub.style.bottom = `${gap}px`;
      show(this.sub);
      this.effortRow.classList.add('is-selected');
    }
    if (select) {
      this.inSub = true;
      this.markSub(Math.max(0, this.subRows.findIndex((r) => r.el.classList.contains('is-current'))));
    }
  }

  drawSub() {
    const effort = this.on.effort();
    this.subRows = effortRows(effort.levels, effort.current).map((r) => {
      const el = h('div.model-menu-row', {
        role: 'menuitemradio',
        onmousedown: (/** @type {MouseEvent} */ e) => e.preventDefault(),
        onmousemove: () => { this.inSub = true; this.markSub(this.subRows.findIndex((x) => x.el === el)); },
        onclick: () => this.chooseEffort(r.level),
      }, h('span.model-menu-text', h('span.model-menu-title', r.title)), icon('check'));
      el.classList.toggle('is-current', r.current);
      return { el, level: r.level };
    });
    replace(this.sub, this.subRows.map((r) => r.el));
  }

  /** 收回子菜单（`now` 不走退场）。 */
  closeSub(now) {
    clearTimeout(this.hoverTimer);
    this.inSub = false;
    if (!this.subOpen) return;
    this.subOpen = false;
    this.effortRow.classList.toggle('is-selected', this.at === this.rows.length);
    if (now) this.sub.hidden = true;
    else hide(this.sub);
  }

  markSub(i) {
    this.subAt = i;
    this.subRows.forEach((r, j) => r.el.classList.toggle('is-selected', this.inSub && i === j));
  }

  /** 选了一档：交出去，不关，勾挪过去、那一行右边的字跟着换。 */
  chooseEffort(level) {
    this.on.chooseEffort(level);
    this.effortValue.textContent = effortLabel(this.on.effort().current);
    const at = this.subAt;
    this.drawSub();
    this.markSub(at);
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
    const onEffort = !this.inSub && this.at === this.rows.length;
    if (this.inSub && (e.key === 'ArrowDown' || e.key === 'ArrowUp')) {
      const i = this.subAt + (e.key === 'ArrowDown' ? 1 : -1);
      if (this.subRows[i]) this.markSub(i);
    } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      const step = e.key === 'ArrowDown' ? 1 : -1;
      let i = this.at + step;
      while (this.rows[i] && !this.rows[i].row.usable) i += step;
      // 列表下面是思考强度那一行
      const last = this.effortEl.hidden ? this.rows.length - 1 : this.rows.length;
      if (i >= 0 && i <= last) this.mark(i, true);
    } else if (onEffort && (e.key === 'ArrowRight' || (e.key === 'Enter' && !e.isComposing))) this.openSub(true);
    else if (this.inSub && (e.key === 'ArrowLeft' || e.key === 'Escape')) {
      this.closeSub(false);
      this.mark(this.rows.length, false);
    } else if (e.key === 'Enter' && !e.isComposing) {
      if (this.inSub) {
        const r = this.subRows[this.subAt];
        if (r) this.chooseEffort(r.level);
      }
      else {
        const row = this.rows[this.at]?.row;
        if (row) this.choose(row);
      }
    } else if (e.key === 'Tab') this.turn(this.page === 'models' ? 'pools' : 'models');
    else if (e.key === 'Escape' && this.query) {
      // 搜索框里有字：先清空，再按一次才关
      this.query = '';
      this.searchInput.value = '';
      this.draw();
      this.mark(Math.max(0, this.rows.findIndex((r) => r.row.current)), true);
    } else if (e.key === 'Escape') this.close();
    else return;
    e.preventDefault();
    e.stopPropagation();
  }
}
