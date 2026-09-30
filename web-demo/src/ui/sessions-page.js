// @ts-check
//! 全部会话（蓝图 `web.md`「全部会话」，照 Claude 网页端的 Chats 页）：左栏「查看全部」、`/sessions` 打开，占对话区那一块（左栏
//! 照旧在）。大标题、搜索、多选、新会话；下面一个顶层会话一行：标题、什么时候开的。全部照 `session.list` 取，不读每个会话的日志；
//! 读过的会话标题照左栏的（没改过标题的拿第一句话顶）；读过的会话写派过几个子代理（一层层往下），有后台任务在跑的写几个在跑。开着时在跑、后台任务的数
//! 变了才重画（`refresh`）。多选照左栏：勾选框、`Shift` 连选、全选、删除（不问）、退出。`Esc` 关上。

import { h, icon, replace } from './dom.js';
import { show, hide, leave } from '../lib/motion.js';
import { t } from '../util/res.js';
import { toggle, range } from '../model/select.js';
import { ago, uuidTime } from '../model/ago.js';

/**
 * @typedef {{list: () => Promise<{session: string, title?: string, parent?: string|null, oneshot?: boolean}[]>,
 *   titleOf: (id: string) => string|null, running: (id: string) => boolean, jobs: (id: string) => number, agents: (id: string) => number,
 *   open: (id: string) => void, newSession: () => void,
 *   removeMany: (ids: string[]) => Promise<string[]>, dropped: (id: string) => void}} PageActions
 */

export class SessionsPage {
  /** @param {PageActions} on */
  constructor(on) {
    this.on = on;
    /** @type {{session: string, title: string|null}[]} */
    this.rows = [];
    this.query = '';
    this.selecting = false;
    this.selected = /** @type {Set<string>} */ (new Set());
    /** 上一次画的每一行在不在跑、几个后台任务：变了才重画 */
    this.live = '';
    this.anchor = /** @type {string|null} */ (null);
    this.search = /** @type {HTMLInputElement} */ (h('input.sessions-search', { type: 'search', placeholder: t('sessions_page.search'), 'aria-label': t('sessions_page.search') }));
    // 边打边筛：输入法在选字时不筛，选定了（`compositionend`）再筛
    const filter = () => {
      this.query = this.search.value.trim().toLowerCase();
      this.draw();
    };
    this.search.addEventListener('input', (e) => { if (!/** @type {InputEvent} */ (e).isComposing) filter(); });
    this.search.addEventListener('compositionend', filter);
    this.tools = h('div.sessions-tools');
    this.title = h('h1.sessions-title', t('sessions_page.title'));
    this.list = h('div.sessions-list');
    this.el = h('section.sessions-page', { hidden: true, 'aria-label': t('sessions_page.title') },
      h('div.sessions-inner', h('header.sessions-head', this.title, this.tools), this.list));
    this.onKey = (/** @type {KeyboardEvent} */ e) => {
      if (e.key !== 'Escape' || e.defaultPrevented) return;
      if (this.selecting) this.select(false);
      else this.close();
    };
  }

  isOpen() {
    return !this.el.hidden && !this.el.classList.contains('is-leaving');
  }

  /** 打开：取全部会话（顶层的，从新到旧），画出来，焦点给搜索框。 */
  async open() {
    this.select(false);
    this.search.value = '';
    this.query = '';
    // 打开时那一排重画一次（界面语言换过的照新的）
    this.toolsMode = null;
    show(this.el);
    document.addEventListener('keydown', this.onKey);
    this.rows = [];
    this.draw();
    try {
      const all = await this.on.list();
      this.rows = all.filter((s) => !s.oneshot && !s.parent).map((s) => ({ session: s.session, title: s.title ?? this.on.titleOf(s.session) }));
    } catch {
      this.rows = [];
    }
    this.draw();
    this.search.focus();
  }

  close() {
    document.removeEventListener('keydown', this.onKey);
    if (this.isOpen()) hide(this.el);
  }

  /** 进、出多选。 */
  select(on) {
    this.selecting = on;
    this.selected = new Set();
    this.anchor = null;
    this.draw();
  }

  /** 右上那一排：平常搜索、多选、新会话；多选时「已选 N 个」和全选、删除、退出（位置不动）。 */
  drawTools() {
    // 平常那一排画一次就留着：每打一个字重建会把搜索框拿下来再放回去，输入法选字就断了（中文只进得去一个字）
    const mode = this.selecting ? 'select' : 'normal';
    if (mode === 'normal' && this.toolsMode === 'normal') return;
    this.toolsMode = mode;
    const icon_ = (name, label, run, cls = '', disabled = false) =>
      h(`button.icon-button.sessions-icon${cls}`, { type: 'button', title: label, 'aria-label': label, disabled, onclick: run }, icon(name));
    const shown = this.visible().map((r) => r.session);
    replace(this.tools, this.selecting
      ? [h('span.sessions-count', t('sidebar.selected', { count: this.selected.size })),
        icon_('check-check', t('sidebar.select_all'), () => { this.selected = new Set(shown); this.draw(); }),
        icon_('trash-2', t('sidebar.delete'), () => this.remove([...this.selected]), '.is-danger', this.selected.size === 0),
        icon_('x', t('sidebar.select_exit'), () => this.select(false))]
      : [h('label.sessions-search-box', icon('search'), this.search),
        h('button.sessions-button', { type: 'button', onclick: () => this.select(true) }, t('sessions_page.select')),
        h('button.sessions-button.is-primary', { type: 'button', onclick: () => { this.close(); this.on.newSession(); } }, icon('plus'), t('sessions_page.new'))]);
  }

  /** 照搜索的字筛出来的几行。 */
  visible() {
    const q = this.query;
    return q ? this.rows.filter((r) => (r.title ?? '').toLowerCase().includes(q)) : this.rows;
  }

  /** 开着时会话在跑、后台任务的数变了：重画（没变的不动，对话区每来一段字都会叫一次）。 */
  refresh() {
    if (!this.isOpen()) return;
    if (this.liveSig() !== this.live) this.draw();
  }

  /** 这几行在不在跑、几个后台任务，拼成一串比。 */
  liveSig() {
    return this.rows.map((r) => `${this.on.running(r.session) ? 1 : 0}${this.on.jobs(r.session)}:${this.on.agents(r.session)}`).join(',');
  }

  /** 一列：一个会话一行，标题、什么时候开的（有后台任务在跑的左边写几个）；多选时前面一个勾选框。 */
  draw() {
    this.live = this.liveSig();
    this.drawTools();
    const now = Date.now();
    const rows = this.visible();
    const order = rows.map((r) => r.session);
    replace(this.list, rows.length ? rows.map((r) => {
      const at = uuidTime(r.session);
      const on = this.selected.has(r.session);
      const click = (/** @type {MouseEvent} */ e) => {
        if (this.selecting) {
          this.selected = e.shiftKey ? range(this.selected, order, this.anchor, r.session) : toggle(this.selected, r.session);
          this.anchor = r.session;
          this.draw();
          return;
        }
        this.close();
        this.on.open(r.session);
      };
      return h(`button.sessions-row${on ? '.is-selected' : ''}`, { type: 'button', dataset: { session: r.session }, onclick: click },
        this.selecting ? h('span.sessions-check', icon(on ? 'square-check' : 'square')) : null,
        this.on.running(r.session) ? h('span.session-run-spinner.sessions-run') : null,
        h(`span.sessions-name${r.title ? '' : '.is-untitled'}`, r.title ?? t('sessions_page.untitled')),
        agents(this.on.agents(r.session)),
        jobs(this.on.jobs(r.session)),
        h('span.sessions-when', at != null ? ago(at, now) : ''));
    }) : [h('div.sessions-empty', t(this.query ? 'sessions_page.no_match' : 'sessions_page.empty'))]);
  }

  /** 删（多选时点删除）：不问，多选当场退出，这几行一起收掉；删成的从表里拿掉，没删成的留着。 */
  async remove(ids) {
    if (!ids.length) return;
    this.selecting = false;
    this.draw();
    const gone = Promise.all(ids.map((id) => new Promise((resolve) => {
      const el = this.list.querySelector(`[data-session="${CSS.escape(id)}"]`);
      if (el instanceof HTMLElement) {
        el.style.setProperty('--h', `${el.offsetHeight}px`);
        leave(el, () => { el.remove(); resolve(undefined); });
      } else resolve(undefined);
    })));
    const done = await this.on.removeMany(ids);
    await gone;
    for (const id of done) this.on.dropped(id);
    this.rows = this.rows.filter((r) => !done.includes(r.session));
    this.selected = new Set();
    this.draw();
  }
}

/** 派过子代理的：机器人加「N 个子代理」（一层层往下都算，跑完了的也算）；没有的不写。 */
function agents(count) {
  return count ? h('span.sessions-agents', icon('bot'), t('sessions_page.agents', { count })) : null;
}

/** 后台任务在跑的：一个呼吸的点加「N 个后台任务」（和框下面那一行一样）；没有的不写。 */
function jobs(count) {
  return count ? h('span.sessions-jobs', h('i.sessions-jobs-dot'), t('sessions_page.jobs', { count })) : null;
}
