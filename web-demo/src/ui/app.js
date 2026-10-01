// @ts-check
//! 整个页面：左栏、对话区、输入框这一块（蓝图 `web.md`「整页」）。状态只有几样：看哪个会话（没有的是一个还没开的
//! 新会话）、左栏收没收起、窄屏上左栏开没开、还没开的新会话上点过的权限级别。收起是这个终端的状态，按账号记在这台设备上
//! （内核的 `storage`，蓝图 `web/architecture.md`「多用户、多终端」；照旧版 `app.js:1130-1155`）。
//!
//! 权限级别点一下换下一级（照 TUI 的 Tab）：发 `session.set_permission_level`，画的是核心记下的
//! （`session.policy_changed`）。还没开的新会话先记着，开了会话、说第一句话之前发给核心（蓝图「框下面那一行」）。
//!
//! 输入框上面的一叠（蓝图 `web.md`「命令列表」「待办」「运行状态行」）：命令列表归输入框，执行交给 `ui/commands.js`；
//! 待办、运行状态行是软件包（`todo`、`pulse`），挂进 `composer.above`，照事件 `session.opened`、`view.changed` 画。
//! 框里的附件是软件包（`attachments`），挂进 `composer.bar`、`composer.head`、`composer.payload`（蓝图「附件」）。

import { h, icon } from './dom.js';
import { show } from '../lib/motion.js';
import { mountList } from '../lib/mount.js';
import { res, t } from '../util/res.js';
import { Sidebar } from './sidebar.js';
import { Chat } from './chat.js';
import { Composer } from './composer.js';
import { Artifacts } from './artifacts.js';
import { runCommand, refusalText, redo, copyTurn, Commands } from './commands.js';
import { project } from '../model/transcript.js';
import { withRecaps, withChanges } from '../model/notes.js';
import { rank } from '../model/session.js';
import { footer, levelLabel, nextLevel, levelParams } from '../model/footer.js';
import { levelOf } from '../model/transcript.js';
import { copy } from '../markdown/build.js';
import { childrenOf, runningDeep } from '../lib/jobs.js';
import { SessionsPage } from './sessions-page.js';
import { listFiles } from '../core/files.js';
import { footerOf } from '../model/model-menu.js';
import { Crumbs, BackButton } from './crumbs.js';
import { pathOf } from '../model/tree.js';

/** 输入历史记在这台设备上的名字（蓝图「输入历史」第 1 条；内核的 `storage` 会带上账号） */
const HISTORY = 'input_history';
/** 左栏收没收，记在这台设备上的名字（内核的 `storage` 会带上账号） */
const COLLAPSED = 'sidebar_collapsed';

export class App {
  /**
   * @param {import('../core/store.js').Store} store
   * @param {import('../core/connection.js').Connection} conn
   * @param {{cwd: string, home: string|null, account?: string}} info 服务 `host`：新会话在哪个目录里干活（起桥的目录）、家目录、登录成的账号
   * @param {any} ctx 软件包 `app` 的上下文：声明挂载位、画挂进来的东西（蓝图 `web/architecture.md`）
   * @param {() => any} lightbox 交回现在的灯箱（软件包 lightbox；没装是 `undefined`）
   */
  constructor(store, conn, info, ctx, lightbox) {
    this.ctx = ctx;
    /** 软件包接进来的：挂载位、现在的灯箱（`rich.js` 的 `Ext`） */
    this.ext = { slots: ctx.slots, lightbox, storage: ctx.storage, account: info.account ?? null, titleOf: (id) => this.titleOf(id) };
    this.store = store;
    this.cwd = info.cwd;
    /** 家目录（`@` 选文件写路径照它写成 `~/…`） */
    this.home = info.home ?? null;
    this.home = info.home;
    this.current = /** @type {string|null} */ (null);
    /** 还没开的新会话上点过的权限级别；`null` 是没点过（核心开出来是什么就是什么）。 */
    this.pendingLevel = /** @type {string|null} */ (null);
    /** 还没开的新会话里选的模型（换模型的菜单、`/model`）：开会话时带上 */
    this.pendingModel = /** @type {string|null} */ (null);
    /** 会话 → 选过、还没生效的模型（下一轮才换）和那时开过几轮：开了新的一轮就照核心推的 */
    this.picked = /** @type {Map<string, {ref: string, turns: number}>} */ (new Map());
    /** 上一次问到的 `model.list`：新会话框下面写默认的那一个；换模型的菜单每次打开再问一次 */
    this.models = /** @type {any} */ (null);
    /** 换级别的命令发出去了还没回应：这时候再点不算。 */
    this.switching = false;
    this.sidebar = new Sidebar({
      newSession: () => this.open(null),
      open: (id) => this.open(id),
      // 临时浮出来时点收起那个按钮是真展开（蓝图「左栏」的「临时浮出」）
      collapse: () => this.collapse(!this.root.classList.contains('is-sidebar-peek')),
      close: () => this.drawer(false),
      theme: () => this.ctx.theme?.next(),
      dark: () => !!this.ctx.theme?.dark(),
      copyId: (id) => copy(id, (text, good) => this.composer.say(text, good)),
      notYet: (name) => this.composer.say(t('sidebar.not_yet', { name })),
      // 改名：空着的是去掉标题（回到照第一句话写）；改成了照推来的 session.meta_changed 画
      rename: async (id, title) => {
        const trimmed = title.trim();
        try {
          await this.store.conn.request('session.set_meta', { session: id, title: trimmed || null });
        } catch (err) {
          this.composer.say(refusalText(err));
        }
      },
      // 置顶、取消置顶（`session.set_meta` 的 `pinned`）：成了照推来的 session.meta_changed 画、重排
      pin: async (id, on) => {
        try {
          await this.store.conn.request('session.set_meta', { session: id, pinned: on });
        } catch (err) {
          this.composer.say(refusalText(err));
        }
      },
      dropped: (id) => {
        // 它下面的子代理的会话跟着它一起没了（核心删父会话连子会话一起挪进回收处）
        const kids = this.descendants(id);
        if (kids.includes(this.current ?? '')) this.current = id;
        for (const kid of kids) this.store.drop(kid);
        this.store.drop(id);
        // 正在看的被删了：开会话表里下一个还在的（一起删的几个里还没收完的跳过）
        if (this.current === id) this.open(this.ranked().find((x) => !this.store.sessions.get(x)?.gone) ?? null);
      },
      // 查看全部：全部会话那一页
      viewAll: () => this.sessionsPage.open(),
      // 挂在一个会话下面的子代理（左栏的树）：标题照派它时的；子会话自己改过标题的照它
      kids: (id) => this.kids(id),
      // 左栏露出来的子代理：读进它们的会话（才知道下面还挂了几个）
      shown: (ids) => { for (const id of ids) if (!this.store.sessions.has(id)) this.store.ensure(id).catch(() => {}); },
      // 删（一个、几个）：照先后一个个删，交回删成的；没删成的提示一句（一个的照原因码，几个的写几个没删）
      removeMany: async (ids) => {
        const done = [];
        const failed = [];
        for (const id of ids) {
          // 它和它下面的子代理的会话：这期间推来的掉队不去补（是因为删才停的）
          for (const kid of [id, ...this.descendants(id)]) this.store.leaving(kid);
          try {
            await this.store.conn.request('session.delete', { session: id });
            done.push(id);
          } catch (err) {
            for (const kid of [id, ...this.descendants(id)]) this.store.leaving(kid, false);
            failed.push(err);
          }
        }
        if (failed.length === 1 && ids.length === 1) this.composer.say(refusalText(failed[0]));
        else if (failed.length) this.composer.say(t('sidebar.bulk_failed', { count: failed.length, reason: refusalText(failed[0]) }));
        return done;
      },
    });
    // Markdown 的代码块照语言分派给挂进来的包（mermaid 这类）；挂的变了，回答整个重画
    ctx.slots.declare('markdown.code', 'keyed');
    this.chat = new Chat(this.home, { say: (text, good) => this.composer.say(text, good) }, {
      copy: (text) => copy(text, (said, good) => this.composer.say(said, good)),
      copyTurn: (turn) => copyTurn(this, turn),
      edit: (text) => redo(this, text),
      redo: () => redo(this, null),
      // 打断那一轮后面那一句：打开后台任务的浮层（软件包 jobs 听这个事件）
      openJobs: () => this.ctx.emit('jobs.open'),
      openSession: (id) => this.open(id),
      // 压缩的进度条走满了：进度那一行收掉，结果那一行露出来（蓝图「压缩的进度」第 5 条）
      compacted: (session) => { if (session) this.store.finishCompaction(session); },
    }, this.ext);
    ctx.slots.watch('markdown.code', () => this.chat.redraw());
    /** 斜杠命令：出厂的一份加软件包登记的（服务 `commands`） */
    this.commands = new Commands(res.commands.commands);
    this.composer = new Composer({
      send: (text, extra) => this.send(text, extra),
      interrupt: () => this.interrupt(),
      cycleLevel: () => this.cycleLevel(),
      command: (spec, words) => runCommand(this, spec, words),
      // 输入历史：记在这台设备上、按账号分开（内核的 `storage`）；读到坏的丢掉
      history: {
        load: () => [].concat(ctx.storage.get(HISTORY, [])).filter((x) => typeof x?.text === 'string' && typeof x?.at === 'number'),
        save: (items) => ctx.storage.set(HISTORY, items),
      },
      session: () => this.current,
      // 换模型的菜单（蓝图「换模型的菜单」）：每次打开问一次 `model.list`；选了下一轮生效
      models: {
        load: () => this.loadModels(),
        current: () => this.modelRef(),
        choose: (row) => this.setModel(row.ref),
      },
      // `@` 选文件（蓝图「`@` 选文件」）：问核心列、找（`fs.list`、`fs.find`，核心施工 W-2），照这个会话的工作目录
      files: (plan, fresh, onUpdate, stale) => listFiles(this.store.conn, this.workdir(), plan, fresh, onUpdate, stale),
      where: () => ({ cwd: this.workdir(), home: this.home }),
    }, () => this.commands.list(), () => ctx.slots.list('composer.payload'));
    // 框里：下面一排左边的按钮、写字的地方上面一排（附件这类软件包画）；跟着话一起发的不画，发的时候交出来
    const failedSlot = (owner, reason) => h('div.slot-failed', t('slot_failed', { owner, reason }));
    ctx.slots.declare('composer.bar', 'list');
    ctx.slots.declare('composer.head', 'list');
    ctx.slots.declare('composer.payload', 'list');
    ctx.slots.declare('composer.footer', 'list');
    ctx.slots.declare('composer.float', 'list');
    ctx.slots.declare('composer.takeover', 'list');
    ctx.effect(() => mountList(this.composer.tools, ctx.slots, 'composer.bar', failedSlot));
    ctx.effect(() => mountList(this.composer.head, ctx.slots, 'composer.head', failedSlot));
    ctx.effect(() => mountList(this.composer.middle, ctx.slots, 'composer.footer', failedSlot));
    ctx.effect(() => mountList(this.composer.float, ctx.slots, 'composer.float', failedSlot));
    ctx.effect(() => mountList(this.composer.takeoverEl, ctx.slots, 'composer.takeover', failedSlot));
    ctx.effect(() => ctx.slots.watch('composer.payload', () => this.composer.syncButton()));
    // 框上面的一叠：挂进 `composer.above` 的（待办这类软件包）在流里、占着地方，下面是运行状态行，再下面是框
    this.dockAbove = h('div.dock-above');
    this.composer.box.before(this.dockAbove);
    // 子代理的会话里：输入框正上方靠左回派它的那一层（蓝图「子代理的会话顶上那条路径」）
    this.back = new BackButton((id) => this.open(id));
    this.composer.box.before(this.back.el);
    ctx.slots.declare('composer.above', 'list');
    ctx.effect(() => mountList(this.dockAbove, ctx.slots, 'composer.above', failedSlot));
    // 子代理的会话顶上那条路径：点了回派它的那一层、进路径上的会话
    this.crumbs = new Crumbs((id) => this.open(id));
    this.scrim = h('button.page-scrim', { type: 'button', 'aria-label': t('sidebar.collapse'), onclick: () => this.drawer(false) });
    // 收起着时最左边那一条：指针进来左栏临时浮出来，离开了滑回去（蓝图「左栏」的「临时浮出」）
    const zone = h('div.sidebar-peek-zone', { style: `--peek-zone: ${res.layout.sidebar_peek.zone}px` });
    this.peekTimer = 0;
    this.root = h('div.app-shell',
      this.scrim,
      zone,
      this.sidebar.el,
      h('main.stage',
        h('div.stage-float',
          h('button.icon-button.sidebar-expand-button', { type: 'button', title: t('sidebar.expand'), onclick: () => this.collapse(false) }, icon('panel-left-open')),
          h('button.icon-button.mobile-menu-button', { type: 'button', title: t('sidebar.expand'), onclick: () => this.drawer(true) }, icon('panel-left'))),
        this.lostBar = h('div.bridge-lost', { hidden: true, role: 'alert' }, t('boot.lost')),
        this.crumbs.el,
        this.chat.el,
        this.stageRight = h('div.stage-right'),
        this.composer.el));
    // 开着的页面，桥重启过换了口令：不再白试重连，对话区顶上挂一条提示（蓝图「连核心」第 9 条）
    this.store.conn.onLost(() => show(this.lostBar));
    // 全部会话：占对话区那一块（左栏「查看全部」、`/sessions`）
    this.sessionsPage = new SessionsPage({
      list: async () => (await this.store.conn.request('session.list', {})).sessions ?? [],
      titleOf: (id) => (this.store.sessions.has(id) ? this.store.summary(id).title : null),
      active: (id) => (this.store.sessions.has(id) ? this.store.summary(id).active : null),
      loaded: (id) => this.store.sessions.has(id),
      running: (id) => (this.store.sessions.has(id) ? this.store.summary(id).running : false),
      jobs: (id) => runningDeep(id, (sid) => this.store.sessions.get(sid)?.events ?? null),
      agents: (id) => this.descendants(id).length,
      open: (id) => this.open(id, true),
      newSession: () => this.open(null),
      removeMany: (ids) => this.sidebar.on.removeMany(ids),
      dropped: (id) => this.sidebar.on.dropped(id),
    });
    this.root.querySelector('.stage')?.append(this.sessionsPage.el);
    // 对话区右边的挂载位：跳转条这类挂进来（软件包 rail）
    ctx.slots.declare('stage.right', 'list');
    // 正文末尾、最后一轮下面（确认和提问了结以后留的这类）
    ctx.slots.declare('chat.tail', 'list');
    ctx.effect(() => mountList(this.chat.tail, ctx.slots, 'chat.tail', failedSlot));
    ctx.effect(() => mountList(this.stageRight, ctx.slots, 'stage.right', failedSlot));
    this.root.classList.toggle('is-sidebar-collapsed', !!ctx.storage.get(COLLAPSED, false));
    const expand = /** @type {HTMLElement} */ (this.root.querySelector('.sidebar-expand-button'));
    this.menuButton = /** @type {HTMLElement} */ (this.root.querySelector('.mobile-menu-button'));
    for (const el of [zone, expand, this.menuButton, this.sidebar.el]) {
      el.addEventListener('mouseenter', () => this.peek(true));
      el.addEventListener('mouseleave', () => this.peek(false));
    }
    // 预览工作区：右边一栏，开着时顶替会话概况；开关按钮在输入框右下角、发送的左边
    this.artifacts = new Artifacts(conn, this.root, (text, good) => this.composer.say(text, good), () => this.schedule(), this.ext);
    this.root.append(this.artifacts.el);
    this.composer.sendButton.before(this.artifacts.toggle);
    // 跳到底部（蓝图「输入框」）：正文离底部远了才露；点了回到最底下、接着跟着最新的
    const syncJump = () => this.composer.showJump(this.chat.distanceToBottom() > res.layout.jump_after);
    this.chat.el.addEventListener('scroll', syncJump, { passive: true });
    this.composer.onJump = () => {
      this.chat.reveal();
      syncJump();
    };
    this.syncJump = syncJump;
    conn.onStatus((s) => this.sidebar.setStatus(s));
    store.on(() => this.schedule());
    addEventListener('resize', () => this.schedule());
    ctx.on('theme.changed', () => this.sidebar.drawThemeButton());
    this.frame = 0;
    /** 会话 → 照回应再画一次的回顾（`/recap` 回应里 `cached` 为真的，蓝图「回顾」第 3 条；只在这一页里） */
    this.recapsAgain = /** @type {Map<string, {after: number, text: string}[]>} */ (new Map());
    /** 会话 → 它下面的子代理（照事件条数记着，`kids`） */
    this.kidCache = new Map();
    // 新会话框下面写默认的模型：起来时问一次（问不到的不写，菜单打开时再问）
    this.loadModels().catch(() => {});
  }

  /** @param {HTMLElement} el */
  mount(el) {
    el.replaceChildren(this.root);
    this.sidebar.setStatus('online');
    this.open(this.ranked()[0] ?? null);
    this.composer.changed();
    this.composer.focus();
  }

  /** 回顾没有新内容（`cached`）：照回应在这一刻的末尾再画一次；同一处同一句不重复。 @param {string} session @param {string} text */
  recapAgain(session, text) {
    const after = this.store.sessions.get(session)?.events.at(-1)?.seq ?? 0;
    const list = this.recapsAgain.get(session) ?? [];
    if (list.some((r) => r.after === after && r.text === text)) return;
    this.recapsAgain.set(session, [...list, { after, text }]);
    this.schedule();
  }

  /** 左栏的先后（`model/session.js` 的 `rank`）：置顶的在最前，别的照最近活动。 */
  ranked() {
    return rank(this.store.order.map((id) => this.store.summary(id))).map((x) => x.session);
  }

  schedule() {
    if (!this.frame) this.frame = requestAnimationFrame(() => { this.frame = 0; this.render(); });
  }

  /**
   * 看一个会话；`null` 是一个还没开的新会话（蓝图「连核心」第 5 条），它不带着上一个新会话的演示待办。
   * 窄屏上顺手收起左栏。
   */
  open(id, listed = false) {
    this.pendingLevel = null;
    this.pendingModel = null;
    this.current = id;
    this.sessionsPage?.close();
    // 没读过的会话第一次打开时才读、订阅：子代理的不进会话表的顶层；全部会话那一页开的老会话进（`listed`）
    if (id) this.store.ensure(id, listed).catch((err) => this.composer.say(refusalText(err)));
    this.chat.reset();
    this.store.view(id);
    this.drawer(false);
    this.render();
    // 软件包跟着正在看的会话走（待办这类）
    this.ctx.emit('session.opened', id);
  }

  /**
   * 说一句话：新会话第一句话发出去时才开会话。`extra` 是跟着发的（附件）。交回核心收没收。
   * 核心拒绝的，在输入框上面提示一句：认得的原因码照 `refusals` 写，别的照核心的原话。
   */
  /** 正在看的会话在哪个目录里干活（开它时的 `cwd`）；还没开的新会话是起桥的目录。 */
  workdir() {
    const events = this.current ? this.store.sessions.get(this.current)?.events ?? [] : [];
    return events.find((e) => e.kind === 'session.created')?.body.cwd ?? this.cwd;
  }

  async send(text, extra = {}) {
    try {
      if (!this.current) {
        this.current = await this.store.create(this.cwd, this.pendingModel);
        this.pendingModel = null;
        this.store.view(this.current);
        // 还是这一段对话：跟着新会话走的软件包（演示待办）跟过去
        this.ctx.emit('session.created', { from: null, to: this.current });
        await this.applyPending();
      }
      await this.store.send(this.current, text, extra);
      return true;
    } catch (err) {
      const why = refusalText(err);
      this.composer.say(why === err.message ? t('refused', { message: why }) : why);
      return false;
    }
  }

  /**
   * 打断（两下 `Esc`、在回答时空着的发送按钮）：排着的一起发出去、接着开下一轮（`queued: send`，蓝图「连核心」第 6 条）。
   * 拒绝的写一句提示。
   */
  async interrupt() {
    if (!this.current) return;
    try {
      await this.store.conn.request('session.interrupt', { session: this.current, queued: 'send' });
    } catch (err) {
      this.composer.say(refusalText(err));
    }
  }

  /**
   * 一个会话下面的子代理（照它的事件，`lib/jobs.js`；读进来了的子代理照它自己的会话认停在半路）。照事件条数记着，没变的不重算
   * （左栏每画一次都要问）。
   */
  kids(id) {
    const events = this.store.sessions.get(id)?.events ?? [];
    const child = (sid) => this.store.sessions.get(sid)?.events ?? null;
    const sig = `${events.length}|${[...this.store.sessions.values()].reduce((n, s) => n + s.events.length, 0)}`;
    const hit = this.kidCache.get(id);
    if (hit?.sig === sig) return hit.kids;
    // 标题照派它时的；子会话自己改过标题的照它（蓝图「左栏」的「子代理的会话」）
    const kids = childrenOf(events, child).map((k) => ({ ...k, title: metaTitle(child(k.session)) ?? k.title }));
    this.kidCache.set(id, { sig, kids });
    return kids;
  }

  /**
   * 该读进来的子代理的会话读进来（蓝图「左栏」的「子代理的会话」）：在跑的子代理（才知道它又派了谁、停没停在半路）、正在看的会话
   * 和它上面几层的子代理。读过的不再读。
   */
  followKids() {
    const want = new Set();
    for (const [id] of this.store.sessions) for (const c of this.kids(id)) if (c.running) want.add(c.session);
    for (const c of this.current ? this.kids(this.current) : []) want.add(c.session);
    for (const id of want) if (!this.store.sessions.has(id)) this.store.ensure(id).catch(() => {});
  }

  /**
   * 派这个会话的会话（子会话 `session.created` 的 `parent`）；没读进来的先读进来（读到了再画一次，路径接上去）。
   * @param {string} id
   */
  parentOf(id) {
    const s = this.store.sessions.get(id);
    if (!s) {
      this.store.ensure(id).catch(() => {});
      return null;
    }
    return s.events.find((e) => e.kind === 'session.created')?.body.parent ?? null;
  }

  /**
   * 一个会话的标题，和左栏写的一样：子代理的照派它时的标题（子会话自己改过标题的照它）；别的照左栏那一项。都没有的是 `null`。
   * @param {string} id
   */
  titleOf(id) {
    const parent = this.parentOf(id);
    const kid = parent ? this.kids(parent).find((k) => k.session === id) : null;
    if (kid) return kid.title;
    return this.store.sessions.has(id) ? this.store.summary(id).title : null;
  }

  /** 挂在一个会话下面的子代理的会话，一层层往下（照派它的会话的事件，`lib/jobs.js`）；同一个不算两遍。 */
  descendants(id, seen = new Set()) {
    const out = [];
    for (const c of this.kids(id)) {
      if (seen.has(c.session)) continue;
      seen.add(c.session);
      out.push(c.session, ...this.descendants(c.session, seen));
    }
    return out;
  }

  /**
   * 空会话（新会话、还没有一句话）：输入框在对话区正中；有了第一句话，输入框从中间滑到底下（蓝图「对话区」的「空会话」）。
   * 位置照 `offsetTop`（对话区自己的像素，整页放大不影响）。
   * @param {boolean} empty
   */
  centerIfEmpty(empty) {
    const stage = /** @type {HTMLElement} */ (this.root.querySelector('.stage'));
    if (stage.classList.contains('is-empty') === empty) return;
    const dock = this.composer.el;
    const before = dock.offsetTop;
    stage.classList.toggle('is-empty', empty);
    const moved = before - dock.offsetTop;
    const reduced = typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
    if (moved && !reduced && stage.isConnected) {
      dock.animate([{ transform: `translateY(${moved}px)` }, { transform: 'none' }], { duration: res.layout.center_move_ms, easing: 'cubic-bezier(0.22, 0.61, 0.36, 1)' });
    }
  }

  /** 收起、展开左栏（桌面），记下来。 */
  collapse(yes) {
    clearTimeout(this.peekTimer);
    this.root.classList.remove('is-sidebar-peek');
    this.root.classList.toggle('is-sidebar-collapsed', yes);
    this.ctx.storage.set(COLLAPSED, yes);
  }

  /**
   * 收起着时指针进出最左边那一条、展开按钮、浮出来的左栏：进来停一小会儿浮出来，出去过一小会儿滑回去
   * （`layout.json` 的 `sidebar_peek`）；在它们之间挪动不算出去。没收起的不管。
   */
  peek(inside) {
    clearTimeout(this.peekTimer);
    // 窄屏（左上角是菜单按钮的时候）：抽屉关着就浮出；宽屏：收起着才浮出。抽屉开着（点过菜单按钮）不管
    const narrow = getComputedStyle(this.menuButton).display !== 'none';
    if (this.root.classList.contains('is-drawer-open')) return;
    if (!narrow && !this.root.classList.contains('is-sidebar-collapsed')) return;
    const { open_ms: open, close_ms: close } = res.layout.sidebar_peek;
    this.peekTimer = window.setTimeout(() => this.root.classList.toggle('is-sidebar-peek', inside), inside ? open : close);
  }

  /** 问一次 `model.list`，记下来（新会话框下面照它写默认的那一个）。 */
  async loadModels() {
    this.models = await this.store.conn.request('model.list', {});
    if (!this.current) this.render();
    return this.models;
  }

  /**
   * 现在的模型引用（菜单打勾、框下面写的）：新会话照选过的、没选过的照默认（`uses.chat`）；开着的会话照选过还没生效的，
   * 开了新的一轮就照核心记着的（`subscribe` 回应、`model.changed`）。
   */
  modelRef() {
    if (!this.current) return this.pendingModel ?? this.models?.uses?.chat ?? null;
    const s = this.store.sessions.get(this.current);
    const picked = this.picked.get(this.current);
    const turns = (s?.events ?? []).filter((e) => e.kind === 'turn.started').length;
    if (picked && turns > picked.turns) this.picked.delete(this.current);
    return this.picked.get(this.current)?.ref ?? s?.model?.ref ?? null;
  }

  /**
   * 换模型（蓝图「换模型的菜单」第 5 条）：开着的会话发 `session.configure`（核心施工 8-10），下一轮生效，框下面当场照选的写；
   * 还没开的新会话先记着，开会话时带上。拒了的照原因码写一句，框下面放回去。
   * @param {string} ref 模型、`@池`、挡位名
   */
  async setModel(ref) {
    const session = this.current;
    if (!session) {
      this.pendingModel = ref;
      this.render();
      return;
    }
    const turns = (this.store.sessions.get(session)?.events ?? []).filter((e) => e.kind === 'turn.started').length;
    // 拒了的放回原来的（上一次选过还没生效的照旧）
    const before = this.picked.get(session);
    this.picked.set(session, { ref, turns });
    this.render();
    try {
      await this.store.conn.request('session.configure', { session, model: ref });
    } catch (err) {
      if (before) this.picked.set(session, before);
      else this.picked.delete(session);
      this.render();
      this.composer.say(refusalText(err));
    }
  }

  /** 点权限级别：换到下一级。开着的会话发给核心，画等 `session.policy_changed`；还没开的新会话先记着（见开头）。 */
  async cycleLevel() {
    if (this.switching) return;
    const session = this.current;
    if (!session) {
      this.pendingLevel = nextLevel(this.pendingLevel ?? footer([], {}).left.level);
      this.render();
      return;
    }
    const events = this.store.sessions.get(session)?.events ?? [];
    await this.setLevel(session, nextLevel(footer(events, {}).left.level));
  }

  /** 新会话刚开：点过的级别和核心开出来的不一样的，说第一句话之前发给核心。 */
  async applyPending() {
    const want = this.pendingLevel;
    this.pendingLevel = null;
    const session = this.current;
    if (!want || !session) return;
    const created = this.store.sessions.get(session)?.events.find((e) => e.kind === 'session.created');
    if (levelOf(created?.body.permission) !== want) await this.setLevel(session, want);
  }

  /** 发 `session.set_permission_level`；拒绝的写一句提示。 */
  async setLevel(session, level) {
    this.switching = true;
    try {
      await this.store.conn.request('session.set_permission_level', { session, ...levelParams(level) });
    } catch (err) {
      this.composer.say(refusalText(err));
    } finally {
      this.switching = false;
    }
  }

  /** 窄屏上左栏是滑出来的一层：开、关。点开的就不是临时浮出的了。 */
  drawer(open) {
    clearTimeout(this.peekTimer);
    if (open) this.root.classList.remove('is-sidebar-peek');
    this.root.classList.toggle('is-drawer-open', open);
  }

  render() {
    const s = this.current ? this.store.sessions.get(this.current) : null;
    const events = s?.events ?? [];
    this.sidebar.render(rank(this.store.order.map((id) => this.store.summary(id))), this.current);
    this.sessionsPage?.refresh();
    const path = this.current ? pathOf(this.current, (id) => this.parentOf(id)).map((id) => ({ session: id, title: this.titleOf(id) })) : [];
    this.crumbs.draw(path);
    this.back?.draw(path.length > 1 ? path[path.length - 2] : null);
    // 回答里的本机地址、结果里的图照这个会话取（工作目录照 `session.created`）
    this.chat.setWhere(this.current, events.find((e) => e.kind === 'session.created')?.body.cwd ?? null);
    const view = project(withChanges(withRecaps(events, this.recapsAgain.get(this.current ?? '') ?? []), s?.changes ?? []), s?.live ?? null, s?.marks, s?.compactStats);
    // 压好了、进度条还没走满：落了盘的那一行先不画（蓝图「压缩的进度」第 5 条）
    const hold = s?.compacting?.note;
    if (hold != null) view.items = view.items.filter((it) => it.seq !== hold);
    this.chat.setCompacting(s?.compacting ?? null, this.current);
    // 先照空不空摆好输入框（居中时对话区没有高度），再画对话：不然第一句话照 0 高算停在哪，被顶到视口上面
    this.centerIfEmpty(view.items.length === 0);
    this.chat.render(view.items);
    this.syncJump?.();
    this.composer.setRunning(!!view.running);
    this.chat.setRunning(!!view.running);
    // 对话区画了一次：照它画的软件包（运行状态行这类）听这个事件；是状态事件，晚起来的包先拿到最后一份
    this.ctx.publish('view.changed', { session: s?.id ?? null, running: view.running, events, live: s?.live ?? null, retry: s?.retry ?? null, queued: view.queued });
    const f = footer(events, s?.limits ?? {}, s?.compactStats, s?.model);
    // 框下面的模型（蓝图「换模型的菜单」第 1 条）：新会话、选过还没生效的、用着池的照引用写；别的照核心报的模型、端点
    const ref = this.modelRef();
    const picked = this.current ? this.picked.get(this.current) : null;
    if (ref && (!this.current || picked || ref.startsWith('@'))) f.left = { ...f.left, ...footerOf(ref) };
    const level = this.current ? null : this.pendingLevel;
    if (level) f.left = { ...f.left, level, label: levelLabel(level) };
    this.composer.drawFooter(f);
    this.artifacts.update(events, this.chat.where);
    this.followKids();
  }
}

/** 会话自己改过的标题（`session.meta_changed` 最后一次写的）；没改过、去掉了的是 `null`。 */
function metaTitle(events) {
  const last = (events ?? []).findLast((e) => e.kind === 'session.meta_changed' && 'title' in e.body);
  return last?.body.title || null;
}
