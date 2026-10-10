// @ts-check
//! 左栏（蓝图 `web.md`「左栏」，照旧版 `web/index.html:52-95`、`styles.css:405-988`）：头像、名字和在线的状态、
//! 新会话、收起；会话表（头上一行「会话」，照旧版组头的样子）；底下换主题。
//!
//! 一项悬停时右边出竖着的三个点（照 Claude Code 的侧栏）：左边一段渐隐，标题淡出去；点开是会话的菜单，样子照旧版
//! （`app.js:2455-2540`）：复制编号、重命名、删除（点了就删，不再问）。
//!
//! 子代理的会话挂在派它的会话下面，一层层往下（「子代理的会话」，照 `model/tree.js` 排）：文件树那样的线画出从属；结束了的收成
//! 「已完成 N 个」；顶层会话的记号那一格是展开、收起整棵的箭头（在跑、没看过的照旧转圈、圆点，指针移上去换成箭头）；子代理下面还
//! 有的，标题前面写 `(N)`，指针移上去换成箭头，点了开、关；整棵收着、里面有在跑的，那一格转圈。记号：子代理是机器人，再往下是
//! 空心的圆（正在看的实心），停在半路的是暂停。顶层最多露 `sidebar_rows` 个，下面一行「查看全部」。
//!
//! 多选（「批量删除」）：「会话」那一行右边的按钮进多选，记号那一格换成勾选框，`Shift` 点连选；「会话」那一行换成已选几个和
//! 全选、删除、退出三个图标按钮，位置不动。
//!
//! 删除一点就开始收（往左淡出、同时收起高度），不等核心；没删成的画回来。
//!
//! 在跑的会话，记号那一格是盲文转圈：整个左栏一个定时器，只在有会话在跑时走（「空闲就是真的空闲」，
//! `00-设计理念.md` 第一节）；帧照墙上的钟取，重画了也接着转，不跳回第一帧。

import { h, icon, replace } from './dom.js';
import { leave } from '../lib/motion.js';
import { res, t } from '../util/res.js';
import { toggle, range } from '../model/select.js';
import { treeRows, capTops } from '../model/tree.js';

/**
 * @typedef {{session: string, title: string|null, running: boolean, unread: boolean}} Item
 * @typedef {{newSession: () => void, open: (id: string) => void, collapse: () => void, close: () => void,
 *   rename: (id: string, title: string) => Promise<void>, pin: (id: string, on: boolean) => Promise<void>, removeMany: (ids: string[]) => Promise<string[]>, dropped: (id: string) => void,
 *   kids: (id: string) => {session: string, title: string|null, running: boolean, paused?: boolean}[], viewAll: () => void,
 *   shown: (ids: string[]) => void,
 *   theme: () => void, dark: () => boolean, copyId: (id: string) => void, notYet: (name: string) => void}} Handlers
 */

export class Sidebar {
  /** @param {Handlers} on */
  constructor(on) {
    /** 正在改名的会话 */
    this.renaming = /** @type {string|null} */ (null);
    /** @type {HTMLInputElement|null} */
    this.renameInput = null;
    /** 多选：在不在多选、勾了哪些、上一次点的（`Shift` 连选从它起） */
    this.selecting = false;
    this.selected = /** @type {Set<string>} */ (new Set());
    this.anchor = /** @type {string|null} */ (null);
    /** 正在删的：不再画（在收的动画里） */
    this.removing = /** @type {Set<string>} */ (new Set());
    /** 树：人点过的开关（会话 → 开着没有）、点开了「已完成 N 个」的会话；这个标签页里记着 */
    this.openTree = /** @type {Map<string, boolean>} */ (new Map());
    this.doneOpen = /** @type {Set<string>} */ (new Set());
    this.on = on;
    this.dot = h('i.status-dot.is-connecting');
    this.status = h('span', t('status.connecting'));
    this.items = h('div.session-items');
    /** 选中的那一块底：会话表里单独一层，换会话时从原来那一项滑过去（蓝图「左栏」的「选中的」） */
    this.pill = h('i.session-pill', { 'aria-hidden': 'true' });
    this.themeButton = h('button.icon-button', { type: 'button', onclick: on.theme });
    /** 底下一行换主题左边：挂载位 `sidebar.actions` 里挂的按钮（设置这类，`ui/app.js` 画进来） */
    this.actions = h('span.sidebar-slot');
    this.groupTitle = h('span', t('sidebar.sessions'));
    this.groupTools = h('span.session-group-tools');
    /** 「会话」那一行、「置顶」那一栏的头：画在会话表里（和会话的行一起排，挪动时一起滑，蓝图「左栏」组头） */
    this.groupHeader = h('div.session-group-header', { dataset: { key: 'group:sessions' } }, icon('message-circle'), this.groupTitle, this.groupTools);
    this.pinnedHeader = h('div.session-group-header.is-pinned', { dataset: { key: 'group:pinned' } }, icon('pin'), h('span', t('sidebar.pinned')));
    /** 左上角的头像、名字：正在看的会话用的人格（`setLook`），点了打开这个人格的编辑页 */
    this.face = h('span.brand-face');
    this.name = h('strong');
    this.identity = h('button.brand-identity', { type: 'button', onclick: () => on.identity?.() },
      this.face, h('div', this.name, h('span', this.dot, this.status)));
    this.setLook(null);
    this.el = h('aside.sidebar',
      h('header.brand-row',
        this.identity,
        h('button.new-chat-button', { type: 'button', title: t('sidebar.new_session'), onclick: on.newSession }, icon('square-pen')),
        h('button.icon-button.sidebar-collapse-button', { type: 'button', title: t('sidebar.collapse'), onclick: on.collapse }, icon('panel-left-close')),
        h('button.icon-button.sidebar-close', { type: 'button', title: t('sidebar.collapse'), onclick: on.close }, icon('x'))),
      h('nav.session-list', h('div.session-track', this.pill, this.items)),
      h('footer.sidebar-footer', h('div.sidebar-actions', this.actions, this.themeButton)));
    this.timer = 0;
    /** 开着菜单的那个会话；没开是 `null`。 */
    this.menuFor = /** @type {string|null} */ (null);
    /** 上一次画的：一样的不重画（流式的字每来一段都会叫一次），开着的菜单、悬停不被打断。 */
    this.drawn = '';
    /** @type {[Item[], string|null]} */
    this.last = [[], null];
    this.drawThemeButton();
    this.drawGroup();
    // 左栏宽了窄了（窄屏的抽屉、改窗口）：那一块跟着这一项的宽
    new ResizeObserver(() => this.placePill()).observe(this.items);
    // 点外面、按 Esc 关上菜单；没开菜单时 Esc 出多选
    document.addEventListener('click', (e) => {
      const target = /** @type {Element} */ (e.target);
      if (this.menuFor && !target.closest?.('.session-menu, .session-menu-button')) this.toggleMenu(null);
    });
    document.addEventListener('keydown', (e) => {
      if (e.key !== 'Escape') return;
      if (this.menuFor) this.toggleMenu(null);
      else if (this.selecting) this.select(false);
    });
  }

  /** 连接的状态：在线、连接中、离线（圆点的颜色照它，蓝图「左栏」的「状态」）。 */
  /**
   * 左上角照会话用的人格画（2026-10-10 项目主人：和对话区她那一轮的头像同步，点了直接开编辑卡片）：有头像的画头像，没有的画名字的第一个
   * 字。无人格的会话不画头像，名称写「Miyu」（`resources/persona.json` 的名字），不能点（同一天项目主人选的）；人格还不知道的时候也这样。
   * @param {import('./chat.js').Look|null} look
   */
  setLook(look) {
    this.face.hidden = !look;
    this.identity.disabled = !look;
    if (!look) {
      replace(this.face);
      this.name.textContent = res.persona.name;
      this.identity.removeAttribute('title');
      return;
    }
    replace(this.face, look.avatar ? h('img.brand-avatar', { src: look.avatar, alt: '' }) : h('span.brand-avatar.is-initial', { 'aria-hidden': 'true' }, [...look.name][0] ?? ''));
    this.name.textContent = look.name;
    this.identity.title = t('sidebar.edit_persona');
  }

  setStatus(status) {
    this.dot.className = `status-dot is-${status}`;
    this.status.textContent = t(`status.${status}`);
  }

  /** 换主题的按钮：深色时画太阳、浅色时画月亮（照旧版 `app.js:684`）。 */
  drawThemeButton() {
    const dark = this.on.dark();
    replace(this.themeButton, icon(dark ? 'sun' : 'moon'));
    this.themeButton.title = t(dark ? 'sidebar.theme_to_light' : 'sidebar.theme_to_dark');
  }

  /** 「会话」那一行：平常右边 ↗（开全部会话那一页）和多选两个按钮，指针移上去才露；多选时换成「已选 N 个」和全选、删除、退出（位置不动）。 */
  drawGroup() {
    const button = (name, label, run, cls = '', disabled = false) =>
      h(`button.icon-button.session-group-button${cls}`, { type: 'button', title: label, 'aria-label': label, disabled, onclick: run }, icon(name));
    this.groupTitle.textContent = this.selecting ? t('sidebar.selected', { count: this.selected.size }) : t('sidebar.sessions');
    const order = this.last[0].map((it) => it.session);
    replace(this.groupTools, this.selecting
      ? [button('check-check', t('sidebar.select_all'), () => { this.selected = new Set(order); this.render(...this.last); }),
        button('trash-2', t('sidebar.delete'), () => this.removeAll(order.filter((id) => this.selected.has(id))), '.is-danger', this.selected.size === 0),
        button('x', t('sidebar.select_exit'), () => this.select(false))]
      : [button('arrow-up-right', t('sidebar.all_sessions'), () => this.on.viewAll(), '.is-select'),
        button('list-checks', t('sidebar.select'), () => this.select(true), '.is-select')]);
    this.el.classList.toggle('is-selecting', this.selecting);
  }

  /**
   * 画会话表。
   * @param {Item[]} items 顶层的会话，照先后，新的在前
   * @param {string|null} current 正在看的会话（可以是子代理的）
   */
  render(items, current) {
    this.last = [items, current];
    const kept = items.filter((it) => !this.removing.has(it.session));
    // 置顶的单开一栏、全部露出（多选时不分栏）；别的顶层最多露几个，正在看的不在里面的排在最后照样露（多选时全列，才勾得到）
    const pinned = this.selecting ? [] : kept.filter((it) => it.pinned);
    const rest = this.selecting ? kept : kept.filter((it) => !it.pinned);
    const cap = this.selecting ? { shown: rest, more: false } : capTops(rest, pinned.some((it) => it.session === current) ? null : current, res.layout.sidebar_rows);
    const tops = [...pinned, ...cap.shown];
    if (this.menuFor && !tops.some((it) => it.session === this.menuFor)) this.menuFor = null;
    // 别处删掉了的不再勾着
    for (const id of [...this.selected]) if (!tops.some((it) => it.session === id)) this.selected.delete(id);
    // 多选时只列顶层的；平常照树排
    const tree = (list) => treeRows(list, (id) => this.on.kids(id), { current, open: this.openTree, done: this.doneOpen });
    const pinnedRows = pinned.length ? tree(pinned) : [];
    const rows = this.selecting
      ? tops.map((it) => ({ kind: /** @type {const} */ ('session'), session: it.session, depth: 0, item: it, hasKids: false, open: false }))
      : [...pinnedRows, ...tree(cap.shown)];
    const sig = JSON.stringify([rows, current, this.menuFor, this.renaming, this.selecting, [...this.selected], cap.more]);
    if (sig !== this.drawn) {
      this.drawn = sig;
      const all = cap.more ? h('button.session-all', { type: 'button', onclick: () => this.on.viewAll() }, t('sidebar.view_all')) : null;
      const draw = (row) => (row.kind === 'session' ? this.item(row, row.session === current) : this.doneRow(row));
      const was = this.positions();
      const hadPinned = this.pinnedHeader.isConnected;
      replace(this.items, [
        pinned.length ? this.pinnedHeader : null, ...pinnedRows.map(draw),
        this.groupHeader, ...rows.slice(pinnedRows.length).map(draw), all]);
      this.slide(was, pinned.length > 0 && !hadPinned);
      this.placeMenu();
      this.drawGroup();
      this.placePill();
      // 露出来的子代理：读进它们的会话，才知道下面还挂了几个
      this.on.shown(rows.flatMap((r) => (r.kind === 'session' && r.depth > 0 ? [r.session] : [])));
    }
    this.spin();
  }

  /** 表里每一行（会话、两栏的头）现在在哪：换先后以前记下，画完照它滑。 */
  positions() {
    const out = new Map();
    for (const el of this.items.children) {
      const key = el instanceof HTMLElement ? el.dataset.session ?? el.dataset.key : null;
      if (key) out.set(key, /** @type {HTMLElement} */ (el).offsetTop);
    }
    return out;
  }

  /**
   * 换了先后（置顶、取消置顶，照最近活动重排）：原来就在的行从原来的位置滑到新位置，别的行跟着让开、收拢；「置顶」那一栏刚出来的
   * 栏头淡入、从上面落下来（蓝图「左栏」组头）。减少动画的不滑。
   * @param {Map<string, number>} was
   * @param {boolean} pinnedAppeared
   */
  slide(was, pinnedAppeared) {
    if (!was.size || matchMedia('(prefers-reduced-motion: reduce)').matches) return;
    const ms = res.layout.sidebar_move_ms;
    const easing = 'cubic-bezier(0.2, 0, 0, 1)';
    for (const el of this.items.children) {
      if (!(el instanceof HTMLElement)) continue;
      const key = el.dataset.session ?? el.dataset.key;
      const from = key ? was.get(key) : undefined;
      if (from != null && from !== el.offsetTop) {
        el.animate([{ transform: `translateY(${from - el.offsetTop}px)` }, { transform: 'none' }], { duration: ms, easing });
      } else if (from == null && key && key !== 'group:pinned') {
        el.animate([{ opacity: 0 }, { opacity: 1 }], { duration: ms, easing });
      }
    }
    if (pinnedAppeared) this.pinnedHeader.animate([{ opacity: 0, transform: 'translateY(-3px)' }, { opacity: 1, transform: 'none' }], { duration: ms, easing });
  }

  /**
   * 选中的那一块挪到正在看的那一项底下：原来就露着的滑过去，原来没露的直接放到位再淡入；正在看的不在表里的淡出。位置照
   * `offset*`（整页放大后的 CSS 像素），和这一项一样大。
   */
  placePill() {
    const row = this.items.querySelector('.session-item.active');
    const pill = this.pill;
    if (!(row instanceof HTMLElement) || this.selecting) {
      pill.classList.remove('is-on');
      return;
    }
    const was = pill.classList.contains('is-on');
    pill.classList.toggle('is-moving', was);
    Object.assign(pill.style, { transform: `translate(${row.offsetLeft}px, ${row.offsetTop}px)`, width: `${row.offsetWidth}px`, height: `${row.offsetHeight}px` });
    pill.classList.add('is-on');
  }

  /**
   * 一项：记号、标题（改名时是一个输入框）；右边三个点，开着菜单的接着画菜单。子代理的缩进；顶层有子代理的，记号那一格上面
   * 一个展开的箭头；子代理下面还有的，标题前面一个 `(N)`（指针移上去换成箭头）。多选时记号那一格是勾选框。
   * @param {{session: string, depth: number, item: any, hasKids: boolean, open: boolean, kidCount?: number, busy?: boolean, last?: boolean, guides?: boolean[]}} row
   * @param {boolean} active
   */
  item(row, active) {
    const it = row.item;
    const id = it.session;
    const title = it.title ?? t('sidebar.untitled');
    const style = `--depth: ${row.depth}`;
    if (this.selecting) {
      const on = this.selected.has(id);
      return h(`div.session-item.is-selecting${on ? '.is-selected' : ''}${active ? '.active' : ''}`, { dataset: { session: id } },
        h('button.session-item-main', { type: 'button', title, 'aria-pressed': String(on), onclick: (e) => this.pick(id, e.shiftKey) },
          h('span.session-lead.session-check', icon(on ? 'square-check' : 'square')),
          h('span.session-copy', h('strong', title))));
    }
    if (this.renaming === id) return this.renameRow(it, title, active, style);
    const open = this.menuFor === id;
    const flip = () => { this.openTree.set(id, !row.open); this.render(...this.last); };
    const foldLabel = t(row.open ? 'sidebar.fold' : 'sidebar.unfold');
    // 顶层：展开、收起整棵的箭头在记号那一格上面（在跑、没看过的照旧转圈、圆点，指针移上去才换成它）
    const fold = row.hasKids && !row.depth
      ? h(`button.session-fold${row.open ? '.is-open' : ''}`, { type: 'button', title: foldLabel, 'aria-expanded': String(row.open), onclick: flip }, icon('chevron-right'))
      : null;
    // 子代理下面还有的：标题前面 `(N)`（在跑的几个），指针移上去换成箭头（点它开、关，不进会话）；一个都没在跑的一直露着箭头
    const kids = row.hasKids && row.depth
      ? h(`span.session-kids${row.open ? '.is-open' : ''}${row.kidCount ? '' : '.is-bare'}`, {
        role: 'button', tabindex: '0', title: foldLabel, 'aria-expanded': String(row.open),
        onclick: (e) => { e.stopPropagation(); flip(); },
        onkeydown: (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); e.stopPropagation(); flip(); } },
      }, row.kidCount ? h('span.session-kids-count', t('sidebar.kids', { count: row.kidCount })) : null, h('span.session-kids-arrow', icon('chevron-right')))
      // 顶层收着的：只写个数（开关是记号那一格上的箭头）
      : row.kidCount ? h('span.session-count', t('sidebar.kids', { count: row.kidCount })) : null;
    const marker = row.busy && !it.running ? h('span.session-lead.session-run-spinner.is-kids', frameNow()) : row.depth ? childLead(it, row.depth, active) : lead(it);
    const quiet = !marker.childElementCount && !marker.textContent;
    const cls = `${active ? '.active' : ''}${open ? '.is-menu-open' : ''}${row.depth ? '.is-child' : ''}${row.hasKids ? '.has-kids' : ''}${row.open ? '.is-open' : ''}${quiet ? '.is-quiet' : ''}`;
    return h(`div.session-item${cls}`, { dataset: { session: id }, style },
      treeLines(row),
      fold,
      h('button.session-item-main', { type: 'button', title, onclick: () => { this.menuFor = null; this.on.open(id); } },
        marker,
        h('span.session-copy', kids, h('strong', title))),
      h('span.session-trailing',
        h('button.session-menu-button', {
          type: 'button', title: t('sidebar.menu'), 'aria-haspopup': 'menu', 'aria-expanded': String(open),
          onclick: () => this.toggleMenu(open ? null : id),
        }, icon('ellipsis-vertical'))),
      open ? menu([
        // 置顶只有顶层会话有（蓝图「左栏」菜单）
        ...(row.depth ? [] : [[t(it.pinned ? 'sidebar.unpin' : 'sidebar.pin'), false, () => this.on.pin(id, !it.pinned)]]),
        [t('sidebar.copy_id'), false, () => this.on.copyId(id)],
        [t('sidebar.rename'), false, () => this.startRename(id)],
        [t('sidebar.delete'), true, () => this.removeAll([id])],
      ], () => this.toggleMenu(null)) : null);
  }

  /** 「已完成 N 个」：一层里结束了的收成这一行，点了列出来、再点收起。 */
  doneRow(row) {
    return h('div.session-done-row', { style: `--depth: ${row.depth}` },
      treeLines(row),
      h(`button.session-done${row.open ? '.is-open' : ''}`, {
        type: 'button', 'aria-expanded': String(row.open),
        onclick: () => {
          if (row.open) this.doneOpen.delete(row.parent);
          else this.doneOpen.add(row.parent);
          this.render(...this.last);
        },
      }, h('span.session-done-mark', icon('check')), t('sidebar.done_kids', { count: row.count }), h('span.session-done-arrow', icon('chevron-right'))));
  }

  /**
   * 改名那一项：标题换成一个输入框，原来的标题填好、全选。`Enter` 改（空着的是去掉标题），`Esc`、点别处不改
   * （蓝图「左栏」的「重命名」）。输入框跨重画留着同一个，打了一半的字不丢。
   */
  renameRow(it, title, active, style) {
    if (!this.renameInput || this.renameInput.dataset.session !== it.session) {
      const input = /** @type {HTMLInputElement} */ (h('input.session-rename', { type: 'text', value: it.title ?? '', 'aria-label': t('sidebar.rename'), dataset: { session: it.session } }));
      let done = false;
      const finish = (commit) => {
        if (done) return;
        done = true;
        this.renaming = null;
        this.renameInput = null;
        if (commit) this.on.rename(it.session, input.value);
        this.render(...this.last);
      };
      input.addEventListener('keydown', (e) => {
        if (e.isComposing || e.keyCode === 229) return;
        if (e.key === 'Enter') {
          e.preventDefault();
          finish(true);
        } else if (e.key === 'Escape') {
          e.preventDefault();
          e.stopPropagation();
          finish(false);
        }
      });
      input.addEventListener('blur', () => finish(false));
      this.renameInput = input;
      requestAnimationFrame(() => {
        input.focus();
        input.select();
      });
    }
    return h(`div.session-item.is-renaming${active ? '.active' : ''}`, { dataset: { session: it.session }, style },
      h('div.session-item-main', lead(it), this.renameInput));
  }

  /** 菜单里点「重命名」：标题换成输入框。 */
  startRename(id) {
    this.renaming = id;
    this.menuFor = null;
    this.render(...this.last);
  }

  /** 开、关菜单（`null` 是关）。 */
  toggleMenu(id) {
    const open = this.items.querySelector('.session-menu');
    this.menuFor = id;
    // 关上：往上收回、淡出，走完再重画（蓝图「动效」）；开另一个的直接换
    if (id === null && open instanceof HTMLElement) leave(open, () => this.render(...this.last));
    else this.render(...this.last);
  }

  /** 进、出多选：出来时勾的都清掉。 */
  select(on) {
    this.selecting = on;
    this.selected = new Set();
    this.anchor = null;
    this.menuFor = null;
    this.renaming = null;
    this.render(...this.last);
    this.drawGroup();
  }

  /** 点一项：勾上、去掉；`Shift` 点把上一次点的和这一项之间的都勾上。 */
  pick(id, shift) {
    const order = this.last[0].map((it) => it.session);
    this.selected = shift ? range(this.selected, order, this.anchor, id) : toggle(this.selected, id);
    this.anchor = id;
    this.render(...this.last);
  }

  /**
   * 删（菜单里删一个、多选删几个）：不问，一点就开始收——这几项一起往左淡出、收起高度，同时照先后请核心删；动画走完直接
   * 拿掉。删成的从会话表里拿掉，没删成的画回来、仍勾着。多选时一个都不剩了退出多选。
   * @param {string[]} ids
   */
  async removeAll(ids) {
    if (!ids.length) return;
    if (this.menuFor) this.toggleMenu(null);
    // 多选时点删除：多选当场退出（勾选框换回记号），同时这几项开始收（蓝图「批量删除」）
    if (this.selecting) this.select(false);
    for (const id of ids) this.removing.add(id);
    const gone = Promise.all(ids.map((id) => new Promise((resolve) => {
      const el = this.items.querySelector(`[data-session="${CSS.escape(id)}"]`);
      if (el instanceof HTMLElement) leave(collapsing(el), () => { el.remove(); resolve(undefined); });
      else resolve(undefined);
    })));
    const done = await this.on.removeMany(ids);
    await gone;
    for (const id of ids) this.removing.delete(id);
    for (const id of done) {
      this.selected.delete(id);
      this.on.dropped(id);
    }
    if (this.selecting && !this.selected.size) this.select(false);
    this.drawn = '';
    this.render(...this.last);
  }

  /** 会话表底下放不下的，菜单往上开（照旧版 `app.js:2466-2470`）。 */
  placeMenu() {
    const m = this.items.querySelector('.session-menu');
    const list = this.items.parentElement;
    if (!m || !list) return;
    if (m.getBoundingClientRect().bottom > list.getBoundingClientRect().bottom - 4) m.classList.add('open-up');
  }

  /** 有会话在跑就走表，没有就停。 */
  spin() {
    const running = this.items.querySelector('.session-run-spinner');
    if (running && !this.timer) this.timer = window.setInterval(() => this.tick(), res.layout.session_spinner_ms);
    if (!running && this.timer) {
      clearInterval(this.timer);
      this.timer = 0;
    }
    this.tick();
  }

  tick() {
    const frame = frameNow();
    for (const el of this.items.querySelectorAll('.session-run-spinner')) el.textContent = frame;
  }
}

/** 这一刻该是哪一帧：照墙上的钟算，所有转圈同一帧。 */
function frameNow() {
  const frames = res.layout.session_spinner;
  return frames[Math.floor(Date.now() / res.layout.session_spinner_ms) % frames.length];
}

/**
 * 会话的菜单：一项一个按钮，危险的红；点了先关菜单再做。
 * @param {[string, boolean, () => void][]} actions 字、是不是危险的、做什么
 */
function menu(actions, close) {
  return h('div.session-menu', { role: 'menu' }, actions.map(([label, danger, run]) =>
    h(`button${danger ? '.is-danger' : ''}`, { type: 'button', role: 'menuitem', onclick: () => { close(); run(); } }, label)));
}

/** 要收掉的一项：记下它现在多高，退场动画照它把高度收成 0（`sidebar.css` 的 `item-out`，蓝图「动效」）。 */
function collapsing(el) {
  el.style.setProperty('--h', `${el.offsetHeight}px`);
  return el;
}

/** 记号那一格：在跑的转圈，没看过的一个圆点，别的空着（在跑的优先）。 */
function lead(it) {
  if (it.running) return h('span.session-lead.session-run-spinner', frameNow());
  if (it.unread) return h('span.session-lead', h('i.session-unread-dot'));
  return h('span.session-lead');
}

/** 子代理的记号：在跑的转圈，停在半路的暂停；别的第一层是机器人，再往下是空心的圆（正在看的实心）。 */
function childLead(it, depth, active) {
  if (it.running) return h('span.session-lead.session-run-spinner', frameNow());
  if (it.paused) return h('span.session-lead.session-paused', { title: t('sidebar.paused') }, icon('circle-pause'));
  if (depth > 1) return h('span.session-lead.session-node', h(`i${active ? '.is-on' : ''}`));
  return h('span.session-lead.session-bot', icon('bot'));
}

/**
 * 文件树那样的线（蓝图「子代理的会话」）：上面几层还没完的，各画一根竖线穿过这一行；自己这一层一段竖线接到上一行，再一小段横线
 * 接到记号，是这一层最后一个的竖线只画到中间（`└`），不是的画到底（`├`）。顶层不画。
 */
function treeLines(row) {
  if (!row.depth) return null;
  const guides = (row.guides ?? []).map((on, i) => (on ? h('i.tree-guide', { style: `--level: ${i + 1}` }) : null));
  return h('span.tree-lines', { 'aria-hidden': 'true' }, ...guides, h(`i.tree-elbow${row.last ? '.is-last' : ''}`, { style: `--level: ${row.depth}` }));
}
