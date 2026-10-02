// @ts-check
//! 会话列表（蓝图 `web.md`「会话列表」，`/sessions` 打开）：浮在输入框上面，和命令列表、输入历史列表同一个位置、同一个样子（带
//! `dock-float`，吉祥物照它站上去）。头一行「会话」、个数和搜索框（打的字进这里，输入法照常能用）；一行四格，一张网格排：记号、
//! 短编号、标题、最近活动；最下面一行暗色写按键。排哪几行、什么先后、记号是什么是 `model/session-list.js` 算的。
//!
//! `↑` `↓` 选（到头绕回去）、`Enter` 打开、`Esc` 关；鼠标悬停选中，点一下打开；点外面关。

import { h, replace } from './dom.js';
import { show, hide } from '../lib/motion.js';
import { res, t } from '../util/res.js';
import { sessionList } from '../model/session-list.js';
import { ago } from '../model/ago.js';

/** @typedef {import('../model/session-list.js').Row} Row */
/** @typedef {import('../model/session-list.js').Item} Item */

export class SessionList {
  /**
   * @param {{rows: () => Promise<Row[]>, current: () => string|null, live: (session: string) => {running: boolean, unread: boolean},
   *   choose: (session: string) => void, closed: () => void}} on
   *   全部顶层会话（`session.list`）、正在看的、在不在跑和看没看过、打开一个、关掉了（焦点回输入框）
   */
  constructor(on) {
    this.on = on;
    this.count = h('span.slist-count');
    this.query = /** @type {HTMLInputElement} */ (h('input.slist-search', {
      // 普通的文字框：搜索框（type=search）浏览器自带一个清除的叉，样子不搭
      type: 'text',
      placeholder: t('session_list.search'),
      oninput: () => this.filter(),
      onkeydown: (/** @type {KeyboardEvent} */ e) => this.key(e),
    }));
    this.list = h('div.slist-list', { role: 'listbox', style: `--rows: ${res.layout.session_list_rows}` });
    this.el = h('div.dock-slist.dock-float', { hidden: true, role: 'dialog' },
      h('div.slist-head', h('strong.slist-title', t('session_list.title')), this.count, this.query),
      this.list,
      h('div.slist-hint', t('session_list.hint')));
    /** @type {Row[]|null} 全部顶层会话；还没取回来的是 `null` */
    this.rows = null;
    /** @type {Item[]} */
    this.items = [];
    this.selected = 0;
    this.pointer = '';
    /** 转圈的定时：有在跑的才开 */
    this.timer = 0;
    /** 在跑、看没看过拼成一串：变了才重画 */
    this.sig = '';
    this.onDown = (/** @type {PointerEvent} */ e) => {
      if (!this.el.contains(/** @type {Node} */ (e.target))) this.close();
    };
  }

  get open() { return !this.el.hidden && !this.el.classList.contains('is-leaving'); }

  /** 打开，带着搜索框里的字（`/sessions 词`）；列表现取。 */
  async show(query = '') {
    this.query.value = query;
    this.rows = null;
    this.filter();
    show(this.el);
    this.query.focus();
    this.query.setSelectionRange(query.length, query.length);
    document.addEventListener('pointerdown', this.onDown, true);
    try {
      this.rows = await this.on.rows();
    } catch (err) {
      console.error(`取不到会话：${/** @type {Error} */ (err).message}`);
      this.rows = [];
    }
    if (this.open) this.filter();
  }

  close() {
    if (!this.open) return;
    document.removeEventListener('pointerdown', this.onDown, true);
    this.stopSpin();
    hide(this.el);
    this.on.closed();
  }

  /** 搜索框里的字变了：重新筛；搜着的选中第一条，没搜的选中当前会话（最上面那一条）。 */
  filter() {
    this.items = this.rows ? sessionList(this.rows, this.on.current(), this.query.value, this.on.live) : [];
    this.selected = 0;
    const total = this.rows?.length ?? 0;
    this.count.textContent = this.query.value.trim() ? t('session_list.count_of', { shown: this.items.length, total }) : t('session_list.count', { count: total });
    this.draw();
  }

  /** 开着时会话在跑、看没看过变了：重画（没变的不动，对话区每来一段字都会叫一次）。 */
  refresh() {
    if (!this.open || !this.rows) return;
    const sig = this.rows.map((r) => { const s = this.on.live(r.session); return `${s.running ? 1 : 0}${s.unread ? 1 : 0}`; }).join('');
    if (sig === this.sig) return;
    const keep = this.items[this.selected]?.session;
    this.items = sessionList(this.rows, this.on.current(), this.query.value, this.on.live);
    this.selected = Math.max(0, this.items.findIndex((it) => it.session === keep));
    this.draw();
  }

  draw() {
    this.sig = (this.rows ?? []).map((r) => { const s = this.on.live(r.session); return `${s.running ? 1 : 0}${s.unread ? 1 : 0}`; }).join('');
    const now = Date.now();
    const empty = this.rows ? t('session_list.no_match') : t('session_list.loading');
    replace(this.list, this.items.length ? this.items.map((it, i) => this.row(it, i, now)) : [h('div.slist-empty', empty)]);
    this.mark();
    if (this.items.some((it) => it.mark === 'running')) this.startSpin();
    else this.stopSpin();
  }

  /** 一行：记号、短编号、标题、最近活动；对上的字高亮。 */
  row(it, i, now) {
    const mark = it.mark === 'running'
      ? h('span.slist-mark.session-run-spinner', frameNow())
      : h('span.slist-mark', it.mark === 'unread' ? h('i.session-unread-dot') : null);
    return h('div.slist-row', {
      role: 'option',
      // 焦点留在搜索框里
      onmousedown: (/** @type {MouseEvent} */ e) => e.preventDefault(),
      onmousemove: (/** @type {MouseEvent} */ e) => this.hover(e, i),
      onclick: () => this.pick(i),
    },
    mark,
    h('span.slist-id', ...marked(it.short, it.hit.short)),
    h('span.slist-name', ...(it.title ? marked(it.title, it.hit.title) : [h('span.slist-untitled', t('sidebar.untitled'))])),
    h('span.slist-ago', it.active ? ago(it.active, now) : ''));
  }

  /** 画选中的那一条，滚进视野（只滚列表自己）。 */
  mark() {
    const rows = /** @type {HTMLElement[]} */ ([...this.list.querySelectorAll('.slist-row')]);
    rows.forEach((r, j) => {
      r.classList.toggle('is-selected', j === this.selected);
      r.setAttribute('aria-selected', String(j === this.selected));
    });
    const row = rows[this.selected];
    if (!row) return;
    const top = row.offsetTop - this.list.offsetTop;
    const bottom = top + row.offsetHeight;
    if (top < this.list.scrollTop) this.list.scrollTop = top;
    else if (bottom > this.list.scrollTop + this.list.clientHeight) this.list.scrollTop = bottom - this.list.clientHeight;
  }

  /** 鼠标真动了才算悬停（列表在鼠标底下动时浏览器也发 `mousemove`）。 */
  hover(e, i) {
    const at = `${e.clientX},${e.clientY}`;
    const moved = at !== this.pointer;
    this.pointer = at;
    if (moved && i !== this.selected) {
      this.selected = i;
      this.mark();
    }
  }

  /** 打开一个：先关掉，再照左栏点开一样开。 */
  pick(i) {
    const it = this.items[i];
    if (!it) return;
    this.close();
    this.on.choose(it.session);
  }

  /** 搜索框里的按键：`↑` `↓` 选（到头绕回去），`Enter` 打开，`Esc` 关；别的照常打进搜索框。 */
  key(e) {
    if (e.isComposing) return;
    const n = this.items.length;
    if (e.key === 'ArrowDown' && n) this.move((this.selected + 1) % n);
    else if (e.key === 'ArrowUp' && n) this.move((this.selected + n - 1) % n);
    else if (e.key === 'Enter') this.pick(this.selected);
    else if (e.key === 'Escape') this.close();
    else return;
    e.preventDefault();
    e.stopPropagation();
  }

  move(i) {
    this.selected = i;
    this.mark();
  }

  /** 转圈：和左栏同一套帧、同一个快慢。 */
  startSpin() {
    if (this.timer) return;
    this.timer = window.setInterval(() => {
      const frame = frameNow();
      for (const el of this.list.querySelectorAll('.session-run-spinner')) el.textContent = frame;
    }, res.layout.session_spinner_ms);
  }

  stopSpin() {
    if (!this.timer) return;
    clearInterval(this.timer);
    this.timer = 0;
  }
}

/** 这一刻转到第几帧（照钟算，和左栏一起转）。 */
function frameNow() {
  const frames = res.layout.session_spinner;
  return frames[Math.floor(Date.now() / res.layout.session_spinner_ms) % frames.length];
}

/** 一段字切成几块，对上的那几块高亮。 @param {string} text @param {[number, number][]} spans */
function marked(text, spans) {
  const out = [];
  let at = 0;
  for (const [a, b] of spans) {
    if (a > at) out.push(text.slice(at, a));
    out.push(h('span.is-mark', text.slice(a, b)));
    at = b;
  }
  if (at < text.length) out.push(text.slice(at));
  return out;
}
