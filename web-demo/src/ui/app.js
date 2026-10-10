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
import { runCommand, refusalText, redo, copyTurn, Commands, revertLatest } from './commands.js';
import { coreSpecs } from '../model/commands.js';
import { rank, startupSession } from '../model/session.js';
import { statusFooter, levelLabel, nextLevel, levelParams } from '../model/footer.js';
import { itemsOf, levelOf } from '../model/entries.js';
import { compactingOf, lastTurn, turnEnded, untouchedTurnOf, artifactEvents } from '../model/view-state.js';
import { copy } from '../markdown/build.js';
import { childrenFromStatus, runningDeepStatus } from '../lib/jobs.js';
import { SessionsPage } from './sessions-page.js';
import { listFiles } from '../core/files.js';
import { footerOf, footerLabel, effortLevels, effortLabel, effortOf, effortChange, defaultModelChange } from '../model/model-menu.js';
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
   * @param {{cwd: string|null, home: string|null, account?: string}} info 服务 `host`：新会话在哪个目录里干活（账号的工作区，握手回应的 `host.workspace`）、家目录、登录成的账号
   * @param {any} ctx 软件包 `app` 的上下文：声明挂载位、画挂进来的东西（蓝图 `web/architecture.md`）
   * @param {() => any} lightbox 交回现在的灯箱（软件包 lightbox；没装是 `undefined`）
   */
  constructor(store, conn, info, ctx, lightbox) {
    this.ctx = ctx;
    /** 软件包接进来的：挂载位、现在的灯箱（`rich.js` 的 `Ext`） */
    this.ext = { slots: ctx.slots, lightbox, storage: ctx.storage, account: info.account ?? null, titleOf: (id) => this.titleOf(id), detail: (session, call) => this.callDetail(session, call) };
    /** 一次调用改了什么（`view.detail`）：同一次调用的结果不会变，照「会话 调用」记着只问一次 @type {Map<string, Promise<any>>} */
    this.details = new Map();
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
    /** 还没开的新会话里选的人格、预设、工作区（软件包 `setup` 经服务 `chat` 改）：开会话时带上，没选的照默认 */
    // 人格 `null` 是没选（照默认的），`false` 是明着不用人格
    this.draft = /** @type {{persona: string|false|null, preset: string|null, cwd: string|null}} */ ({ persona: null, preset: null, cwd: null });
    /** 会话用的人格画成什么样（名字、头像）：软件包 `setup` 经 `chat.look` 交来；没人交的照旧的一份（`resources/persona.json`） */
    this.looker = /** @type {((session: string|null) => import('./chat.js').Look|null|undefined)|null} */ (null);
    /** 上一次交给对话区、输入框的（一样的不再交） */
    this.lookSig = '';
    /** 会话 → 选过、还没生效的模型（下一轮才换）和那时开过几轮：开了新的一轮就照核心推的 */
    this.picked = /** @type {Map<string, {ref: string, turns: number}>} */ (new Map());
    /** 会话（新会话是空的）→ 模型 → 选过、还没生效的思考强度和那时开过几轮（核心施工 8-18：强度是这个会话里这一个模型的一格） */
    // 选了还没写好的思考强度（模型 → 档）：写好了重问 `model.list`，回来之前照它写
    this.pickedEffort = /** @type {Map<string, string|null>} */ (new Map());
    /** 上一次问到的 `model.list`：新会话框下面写默认的那一个；换模型的菜单每次打开再问一次 */
    this.models = /** @type {any} */ (null);
    /** 换级别的命令发出去了还没回应：这时候再点不算。 */
    this.switching = false;
    this.sidebar = new Sidebar({
      newSession: () => this.open(null),
      open: (id) => this.open(id),
      // 左上角的头像：打开这个会话的人格的编辑页（软件包 setup 听这个事件，2026-10-10 项目主人）
      identity: () => this.ctx.emit('persona.open'),
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
    }, this.ext);
    ctx.slots.watch('markdown.code', () => this.chat.redraw());
    // 正文里交给软件包画的条目（确认和提问了结以后留的，按 `asking`）；挂的变了，正文重画
    ctx.slots.declare('chat.item', 'keyed');
    ctx.slots.watch('chat.item', () => this.chat.redraw());
    /** 斜杠命令：出厂的一份加软件包登记的（服务 `commands`） */
    this.commands = new Commands(res.commands.commands);
    this.composer = new Composer({
      send: (text, extra) => this.send(text, extra),
      interrupt: () => this.interrupt(),
      cycleLevel: () => this.cycleLevel(),
      command: (spec, words) => runCommand(this, spec, words),
      // 命令列表开了：照正在看的会话重问核心认哪些（预设、记忆开关换了，下一次打开就是新的）
      commandsOpened: () => this.refreshCommands(),
      // 空会话里核心认、要等会话的命令名（打了不当话发）
      laterCommands: () => this.commands.later,
      // `/help`：核心认的全部（不照会话筛）
      allCommands: async () => this.commands.listWith(coreSpecs((await this.store.conn.request('command.catalog', {}))?.commands ?? [])),
      // 输入历史：记在这台设备上、按账号分开（内核的 `storage`）；读到坏的丢掉
      history: {
        load: () => [].concat(ctx.storage.get(HISTORY, [])).filter((x) => typeof x?.text === 'string' && typeof x?.at === 'number'),
        save: (items) => ctx.storage.set(HISTORY, items),
      },
      session: () => this.current,
      // 换模型的菜单（蓝图「换模型的菜单」）：每次打开问一次 `model.list`；选了下一轮生效
      // 会话列表（`/sessions`，蓝图「会话列表」）：全部顶层会话照会话表；在不在跑、看没看过：读过日志的照日志，没读过的照会话表
      sessions: {
        rows: async () => this.store.index.all().filter((s) => !s.oneshot && !s.parent).map((s) => {
          const it = this.store.summary(s.session);
          return { session: s.session, title: this.titleOf(s.session), pinned: it.pinned, active: it.active };
        }),
        current: () => this.current,
        live: (/** @type {string} */ id) => {
          const it = this.store.summary(id);
          return { running: it.running, unread: it.unread };
        },
        choose: (/** @type {string} */ id) => this.open(id, true),
      },
      models: {
        load: () => this.loadModels(),
        cached: () => this.models,
        current: () => this.modelRef(),
        choose: (row) => this.setModel(row.ref),
        effort: () => {
          const ref = this.modelRef();
          const model = ref && !ref.startsWith('@') ? ref : null;
          return { model, levels: effortLevels(this.models, model), current: this.effortLevel() };
        },
        chooseEffort: (level) => this.setEffort(level),
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
    // 左栏底下一行靠右、换主题按钮左边的按钮（设置页这类软件包画，蓝图「左栏」的「底下一行」）
    ctx.slots.declare('sidebar.actions', 'list');
    ctx.effect(() => mountList(this.sidebar.actions, ctx.slots, 'sidebar.actions', failedSlot));
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
      this.stageEl = h('main.stage',
        h('div.stage-float',
          h('button.icon-button.sidebar-expand-button', { type: 'button', title: t('sidebar.expand'), onclick: () => this.collapse(false) }, icon('panel-left-open')),
          h('button.icon-button.mobile-menu-button', { type: 'button', title: t('sidebar.expand'), onclick: () => this.drawer(true) }, icon('panel-left'))),
        this.lostBar = h('div.bridge-lost', { hidden: true, role: 'alert' }, t('boot.lost')),
        this.crumbs.el,
        // 对话区左上角浮着的一小条（挂载位 `stage.info`：软件包 `setup` 画人格、工作区）
        this.stageInfo = h('div.stage-info'),
        this.chat.el,
        this.stageRight = h('div.stage-right'),
        this.composer.el));
    // 左上角浮着的那一条露着没有、多高：露着时正文从它下面起、顶上淡出（`.has-info`、`--stage-info-h`，styles/chat.css）
    const infoSize = new ResizeObserver(() => {
      const tall = this.stageInfo.getBoundingClientRect().height;
      this.stageEl.classList.toggle('has-info', tall > 0);
      this.stageEl.style.setProperty('--stage-info-h', `${Math.round(tall)}px`);
    });
    infoSize.observe(this.stageInfo);
    // 开着的页面，桥重启过换了口令：不再白试重连，对话区顶上挂一条提示（蓝图「连核心」第 9 条）
    this.store.conn.onLost(() => show(this.lostBar));
    // 全部会话：占对话区那一块（左栏「查看全部」、`/sessions`）
    this.sessionsPage = new SessionsPage({
      // 全部会话照会话表（核心推的跟着变）；标题、最近活动、在不在跑读进来了的照日志，没读的照会话表
      list: async () => this.store.index.all(),
      titleOf: (id) => this.store.summary(id).title,
      active: (id) => this.store.summary(id).active,
      running: (id) => this.store.summary(id).running,
      jobs: (id) => this.jobsDeep(id),
      agents: (id) => this.descendants(id).length,
      open: (id) => this.open(id, true),
      newSession: () => this.open(null),
      removeMany: (ids) => this.sidebar.on.removeMany(ids),
      dropped: (id) => this.sidebar.on.dropped(id),
    });
    this.root.querySelector('.stage')?.append(this.sessionsPage.el);
    // 别处（终端、别的页面）删掉了读进来的会话：和这里删的一样收掉，正在看的换到下一个
    this.store.removed = (id) => this.sidebar.on.dropped(id);
    // 排着的话被退回了（视图流推到 `withdrawn`，打断时没听到的那几条）：正在看的会话的放回输入框（核心 `view.md`「种类」的 `user`）
    // 后台开始提前压缩了（核心 6-11 三补）：正在看的会话弹一句，正文里不画（2026-10-10 项目主人）
    this.store.preparing = (session) => { if (session === this.current) this.composer.say(t('notes.compaction_started')); };
    this.store.withdrawn = (id, text) => {
      if (id === this.current && text) this.composer.putBack(text);
    };
    // 对话区右边的挂载位：跳转条这类挂进来（软件包 rail）
    ctx.slots.declare('stage.right', 'list');
    // 正文末尾、最后一轮下面（确认和提问了结以后留的这类）
    ctx.slots.declare('chat.tail', 'list');
    ctx.effect(() => mountList(this.chat.tail, ctx.slots, 'chat.tail', failedSlot));
    ctx.effect(() => mountList(this.stageRight, ctx.slots, 'stage.right', failedSlot));
    ctx.slots.declare('stage.info', 'list');
    ctx.effect(() => mountList(this.stageInfo, ctx.slots, 'stage.info', failedSlot));
    this.root.classList.toggle('is-sidebar-collapsed', !!ctx.storage.get(COLLAPSED, false));
    // 按页读（核心 9-6 下）：往上滚到顶自动读更早的一页（2026-10-08 项目主人定）
    this.chat.el.addEventListener('scroll', () => this.maybeOlder(), { passive: true });
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
    // 打断时她还没开始做事的那一轮：结束了就撤掉、把话放回框里（蓝图「按键」两下 `Esc`）
    store.on(() => this.takeBackIfEnded());
    addEventListener('resize', () => this.schedule());
    ctx.on('theme.changed', () => this.sidebar.drawThemeButton());
    this.frame = 0;
    /** 会话 → 照回应再画一次的回顾（`/recap` 回应里 `cached` 为真的，蓝图「回顾」第 3 条；只在这一页里） */
    this.recapsAgain = /** @type {Map<string, {after: string, text: string}[]>} */ (new Map());
    // 新会话框下面写默认的模型：起来时问一次（问不到的不写，菜单打开时再问）
    this.loadModels().catch(() => {});
    // 模型、供应商、池的配置变了（设置页、第一次引导、别的头、命令行改的）：重读模型列表，新会话框下面照新的默认写（2026-10-10 项目主人：
    // 引导里选了模型，框下面照旧写原来那个）
    this.store.conn.onPush((method, params) => {
      if (method !== 'config.changed') return;
      if (Object.keys(params?.keys ?? {}).some((k) => /^(models|providers|pools)\./.test(k))) this.loadModels().catch(() => {});
    });
  }

  /** @param {HTMLElement} el */
  mount(el) {
    el.replaceChildren(this.root);
    this.sidebar.setStatus('online');
    // 打开页面时进哪个会话照共用的 `ui.startup`（蓝图「连核心」第 4 条）：先是新会话，读到 `recent` 再换到最近的那个；
    // 这一会儿人已经动了（开了别的会话、打了字）的不换
    this.open(null);
    this.store.conn.request('config.get', { keys: ['ui.startup'] }).catch(() => null).then((reply) => {
      const id = startupSession(reply, this.ranked());
      if (id && this.current === null && !this.composer.input.value) this.open(id);
    });
    this.composer.changed();
    this.composer.focus();
  }

  /** 回顾没有新内容（`cached`）：照回应在这一刻的末尾再画一次；同一处同一句不重复。 @param {string} session @param {string} text */
  recapAgain(session, text) {
    const s = this.store.sessions.get(session);
    // 排在这一刻最后一条条目后面（编号）
    const after = s?.view.list.at(-1)?.id ?? '';
    const list = this.recapsAgain.get(session) ?? [];
    if (list.some((r) => r.after === after && r.text === text)) return;
    this.recapsAgain.set(session, [...list, { after, text }]);
    this.schedule();
  }

  /**
   * 滚到离顶不远（`layout.json` 的 `older_px`）、还有更早的、没在读：读更早的一页。读之前记下离底边多远，画完照它放回去，正在看的不跳。
   */
  maybeOlder() {
    const s = this.current ? this.store.sessions.get(this.current) : null;
    const el = this.chat.el;
    if (!s?.more || s.older || el.scrollTop > res.layout.older_px) return;
    this.keepFromBottom = el.scrollHeight - el.scrollTop;
    this.store.older(s.id).catch((err) => {
      this.keepFromBottom = null;
      this.composer.say(refusalText(err));
    });
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
    this.draft = { persona: null, preset: null, cwd: null };
    this.current = id;
    this.sessionsPage?.close();
    // 没读过的会话第一次打开时才读、订阅：子代理的不进会话表的顶层；全部会话那一页开的老会话进（`listed`）
    if (id) this.store.ensure(id, listed).catch((err) => this.composer.say(refusalText(err)));
    this.chat.reset();
    // 换会话当场摆好输入框，不播从中间滑下去的动画（2026-10-10 项目主人：切换会话每次都动一下）
    this.centerIfEmpty(!id, false);
    this.syncLook();
    this.store.view(id);
    this.drawer(false);
    this.render();
    // 软件包跟着正在看的会话走（待办这类）
    this.ctx.emit('session.opened', id);
  }

  /**
   * 说一句话：新会话第一句话发出去时才开会话。`extra` 是跟着发的（附件）。交回核心收没收。
   * 核心拒绝的，在输入框上面提示核心给的那一句。
   */
  /**
   * 正在看的会话在哪个目录里干活：会话状态里的工作区（核心 9-7，订阅回应的、之后换了的）；还没开的新会话是选的工作区，
   * 没选的是账号的工作区。
   */
  workdir() {
    if (!this.current) return this.draft.cwd ?? this.cwd;
    const s = this.store.sessions.get(this.current);
    return s?.view.status?.workspace?.cwd ?? s?.base?.workspace?.cwd ?? this.cwd;
  }

  /** 开到第几轮（换模型照它认选的生效没有）：照最后一条的回合。 @param {any} s */
  turnsOf(s) {
    return s ? lastTurn(s.view.list) : 0;
  }

  /** 一个会话里在跑的后台任务一共几个（连子孙）：照会话状态的（核心算好的整棵树）。 @param {string} id */
  jobsDeep(id) {
    return runningDeepStatus(id, (sid) => this.store.sessions.get(sid)?.view.status ?? null);
  }

  /** 正在看的会话现在的权限级别：照会话状态的。 @param {string} session */
  levelNow(session) {
    const s = this.store.sessions.get(session);
    return levelOf(s?.view.status?.permission ?? s?.base?.permission);
  }

  /**
   * 一次编辑、写入改了什么（`view.detail`，核心 9-6 三补）：完整的差异带行号。问不到的（旧核心、没有这次调用）交 `null`，
   * 那一步照参数比的照旧。
   * @param {string} session @param {string} call
   */
  callDetail(session, call) {
    const key = `${session} ${call}`;
    if (!this.details.has(key)) this.details.set(key, this.store.conn.request('view.detail', { session, call }).catch(() => null));
    return /** @type {Promise<any>} */ (this.details.get(key));
  }

  /**
   * 照正在看的会话问核心认哪些斜杠命令（`command.catalog`，核心 O-6 补）：列出来的打了不会被拒；空会话里不列核心的（核心的命令要造好的
   * 会话）。问回来时会话已经换了的不要；命令列表开着的照新的重筛。
   */
  async refreshCommands() {
    const session = this.current;
    if (!session) {
      // 空会话里不列核心的，可打了它们的名字要认得出来：不带会话问一遍，记下名字、别名
      this.commands.setCore([]);
      const all = await this.store.conn.request('command.catalog', {}).catch(() => null);
      if (this.current === null && all) this.commands.setLater(new Set((all.commands ?? []).flatMap((c) => [c.name, ...(c.aliases ?? [])].map((n) => String(n).toLowerCase()))));
      return;
    }
    this.commands.setLater(new Set());
    const got = await this.store.conn.request('command.catalog', { session }).catch(() => null);
    if (this.current !== session || !got) return;
    this.commands.setCore(coreSpecs(got.commands ?? []));
    if (this.composer.menu.open) this.composer.menu.update(this.composer.input.value);
  }

  /** 还没开的新会话改人格、预设、工作区（软件包 `setup`）：告诉软件包（`draft.changed`），重画（`@` 照它列文件）。 @param {Partial<{persona: string|false|null, preset: string|null, cwd: string|null}>} patch */
  /**
   * 正在看的会话的人格画成什么样，交给对话区（她那一轮的头）和输入框（占位字）。`looker` 交 `undefined` 的是还不知道（人格列表没读完），
   * 照旧不动；`null` 是无人格（2026-10-10 项目主人：无人格的会话不应该显示头像和 AI 名称）。
   */
  syncLook() {
    const look = this.looker ? this.looker(this.current) : res.persona;
    if (look === undefined) return;
    const sig = JSON.stringify(look);
    if (sig === this.lookSig) return;
    this.lookSig = sig;
    this.chat.setLook(look);
    this.sidebar.setLook(look);
    this.composer.setName(look?.name ?? null);
    // 软件包跟着它变（跟着人格的主题色，`theme-persona`）：状态事件，晚起来的包先拿到最后一份；`null` 是无人格
    this.ctx.publish('look.changed', look);
    this.schedule();
  }

  /** @param {(session: string|null) => import('./chat.js').Look|null|undefined} fn */
  setLooker(fn) {
    this.looker = fn;
    this.syncLook();
  }

  setDraft(patch) {
    this.draft = { ...this.draft, ...patch };
    this.ctx.emit('draft.changed', { ...this.draft });
    this.schedule();
  }

  /**
   * 换这个会话在哪干活（`/workspace`、网页里选，核心 9-7 的 `session.set_workspace`）：会话记着、别的头照推送跟着换；画照推来的
   * `session.workspace_changed`。太宽的核心退回账号的工作区、回应写实际的：告诉软件包（`workdir.adjusted`）。还没开的新会话改选的工作区。
   * 交回实际的目录；核心拒了的（不在、不是目录、在数据根里）抛出来。
   * @param {string|null} session @param {string} cwd
   */
  async setWorkdir(session, cwd) {
    if (!session) {
      this.setDraft({ cwd });
      return cwd;
    }
    const got = await this.store.conn.request('session.set_workspace', { session, cwd });
    if (typeof got?.cwd === 'string' && got.cwd !== cwd) this.ctx.emit('workdir.adjusted', { session, asked: cwd, cwd: got.cwd });
    // 太宽、照人设的用着（核心 9-7 补）：提醒一句，不改
    if (got?.wide === true) this.ctx.emit('workdir.wide', { session, cwd: got.cwd ?? cwd });
    this.schedule();
    return got?.cwd ?? cwd;
  }

  async send(text, extra = {}) {
    try {
      if (!this.current) {
        const asked = this.draft.cwd;
        this.current = await this.store.create(asked ?? this.cwd, this.pendingModel, this.draft.persona, this.draft.preset, !!asked);
        if (this.store.createdWide === this.current) this.ctx.emit('workdir.wide', { session: this.current, cwd: asked });
        this.pendingModel = null;
        this.draft = { persona: null, preset: null, cwd: null };
        this.store.view(this.current);
        // 还是这一段对话：跟着新会话走的软件包（演示待办）跟过去
        this.ctx.emit('session.created', { from: null, to: this.current });
        await this.applyPending();
      }
      // 工作区是会话的属性（核心 9-7）：说话不再带 `cwd`
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
    const session = this.current;
    // 她还没开始做事（没写正文、没调工具）、又没有排着的话：记下这一轮，结束了撤掉、把话放回框里（2026-10-07 项目主人定，终端同一条）
    const s = this.store.sessions.get(session);
    const turn = this.queuedNow?.length || !s ? null : untouchedTurnOf(s.view.list, s.view.status);
    try {
      await this.store.conn.request('session.interrupt', { session, queued: 'send' });
      if (turn != null) {
        this.takeBack = { session, turn };
        this.takeBackIfEnded();
      }
    } catch (err) {
      this.composer.say(refusalText(err));
    }
  }

  /** 记着要撤的那一轮结束了（这一轮有了收尾那一条）：撤掉、把话放回框里；撤不成的什么都不做。 */
  takeBackIfEnded() {
    const pending = this.takeBack;
    if (!pending) return;
    const s = this.store.sessions.get(pending.session);
    if (!s || !turnEnded(s.view.list, pending.turn)) return;
    this.takeBack = null;
    revertLatest(this, pending.session).catch(() => {});
  }

  /**
   * 一个会话下面的子代理：照它会话状态里的任务表（`lib/jobs.js`）。标题照派它时的，子会话自己改过标题的照会话表（蓝图「左栏」的
   * 「子代理的会话」）。
   */
  kids(id) {
    const own = this.store.sessions.get(id);
    return childrenFromStatus(own?.view.status?.jobs).map((k) => ({ ...k, title: this.store.index.get(k.session)?.title ?? k.title }));
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

  /** 派这个会话的会话：照会话表里子会话的 `parent`；会话表里没有的是 `null`。 @param {string} id */
  parentOf(id) {
    return this.store.index.get(id)?.parent ?? null;
  }

  /**
   * 一个会话的标题，和左栏写的一样：子代理的照派它时的标题（子会话自己改过标题的照它）；别的照左栏那一项。都没有的是 `null`。
   * @param {string} id
   */
  titleOf(id) {
    const parent = this.parentOf(id);
    const kid = parent ? this.kids(parent).find((k) => k.session === id) : null;
    if (kid) return kid.title;
    return this.store.summary(id).title;
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
   * 空会话（还没开的新会话）：输入框在对话区正中；发出第一句话、会话开了，输入框从中间滑到底下（蓝图「对话区」的「空会话」）。开着的会话
   * 不管读没读进来都在底下（原来照有没有条目算，没读过的会话读的那一下是空的，切过去先居中再滑下来，2026-10-10 项目主人）。
   * 位置照 `offsetTop`（对话区自己的像素，整页放大不影响）。
   * @param {boolean} empty @param {boolean} [animate] 换会话的不播
   */
  centerIfEmpty(empty, animate = true) {
    const stage = /** @type {HTMLElement} */ (this.root.querySelector('.stage'));
    if (stage.classList.contains('is-empty') === empty) return;
    const dock = this.composer.el;
    const before = dock.offsetTop;
    stage.classList.toggle('is-empty', empty);
    const moved = before - dock.offsetTop;
    const reduced = typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
    if (animate && moved && !reduced && stage.isConnected) {
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
    // 框下面那一截照列表写显示名：列表来了重画一次
    this.schedule();
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
    const turns = this.turnsOf(s);
    if (picked && turns > picked.turns) this.picked.delete(this.current);
    return this.picked.get(this.current)?.ref ?? s?.model?.ref ?? null;
  }

  /**
   * 换模型（蓝图「换模型的菜单」第 5 条）：开着的会话发 `session.configure`（核心施工 8-10），下一轮生效，框下面当场照选的写；
   * 还没开的新会话先记着，开会话时带上。拒了的照原因码写一句，框下面放回去。换成了的同时记成新会话的默认（`rememberModel`）。
   * @param {string} ref 模型或 `@池`
   */
  async setModel(ref) {
    const session = this.current;
    if (!session) {
      this.pendingModel = ref;
      this.render();
      await this.rememberModel(ref);
      return;
    }
    const turns = this.turnsOf(this.store.sessions.get(session));
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
      return;
    }
    await this.rememberModel(ref);
  }

  /** 手动选的模型记成新会话的默认（个人设置的 `models.chat`），写好了重问 `model.list`；记不下的写一句，这个会话照样换了。 */
  async rememberModel(ref) {
    try {
      await this.store.conn.request('config.set', defaultModelChange(ref));
      await this.loadModels().catch(() => {});
    } catch (err) {
      this.composer.say(refusalText(err));
    }
  }

  /**
   * 现在这个模型的思考强度（`null` 是默认）：选了还没写好的照选的；别的照 `model.list` 的 `facts.effort`（个人设置里写了的照写，
   * 系统配置的、没写的是默认）。强度是配置（核心施工 8-18 补），不分会话。
   */
  effortLevel() {
    const ref = this.modelRef();
    if (!ref || ref.startsWith('@')) return null;
    if (this.pickedEffort.has(ref)) return this.pickedEffort.get(ref) ?? null;
    return effortOf(this.models, ref).level;
  }

  /**
   * 换思考强度（蓝图「换模型的菜单」第 2 条，2026-10-02 项目主人改定）：写进个人设置（`config.set`，键名照抄 `facts.effort.key`），
   * 所有会话下一轮都照它；选默认的删掉个人这一项。框下面当场照选的写，写好了重问 `model.list`；拒了的放回去、写一句。
   * @param {string|null} level `null` 是默认
   */
  async setEffort(level) {
    const model = this.modelRef();
    const key = model ? effortOf(this.models, model).key : null;
    if (!model || !key) return;
    this.pickedEffort.set(model, level);
    this.render();
    try {
      await this.store.conn.request('config.set', effortChange(key, level));
      await this.loadModels().catch(() => {});
    } catch (err) {
      this.composer.say(refusalText(err));
    }
    this.pickedEffort.delete(model);
    this.render();
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
    await this.setLevel(session, nextLevel(this.levelNow(session)));
  }

  /** 新会话刚开：点过的级别和核心开出来的不一样的，说第一句话之前发给核心。 */
  async applyPending() {
    const want = this.pendingLevel;
    this.pendingLevel = null;
    const session = this.current;
    if (!want || !session) return;
    if (this.levelNow(session) !== want) await this.setLevel(session, want);
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
    this.sidebar.render(rank(this.store.order.map((id) => this.store.summary(id))), this.current);
    this.sessionsPage?.refresh();
    this.composer?.sessionList.refresh();
    const path = this.current ? pathOf(this.current, (id) => this.parentOf(id)).map((id) => ({ session: id, title: this.titleOf(id) })) : [];
    this.crumbs.draw(path);
    this.back?.draw(path.length > 1 ? path[path.length - 2] : null);
    // 回答里的本机地址、结果里的图照这个会话取（工作目录照 `session.created`）
    this.chat.setWhere(this.current, this.current ? this.workdir() : null);
    // 子会话的父会话照会话表（按页读时 `session.created` 可能还在没读的页里）
    const parent = this.current ? this.store.index.get(this.current)?.parent ?? null : null;
    const again = this.recapsAgain.get(this.current ?? '') ?? [];
    // 条目、会话状态换成正文（核心 9-8，蓝图「照条目画」）
    const view = withLocalRecaps(itemsOf(s?.view.list ?? [], s?.view.status ?? null, { account: this.ext.account, parent }), again);
    this.chat.setCompacting(s ? compactingOf(s.view.list, s.view.status) : null, this.current);
    // 先照空不空摆好输入框（居中时对话区没有高度），再画对话：不然第一句话照 0 高算停在哪，被顶到视口上面
    this.centerIfEmpty(!this.current);
    this.chat.render(view.items);
    // 更早的一页刚接在前面：正在看的那一句不跳（照离底边的距离放回去）；最上面那一行照读到哪写
    if (this.keepFromBottom != null && !s?.older) {
      this.chat.el.scrollTop = this.chat.el.scrollHeight - this.keepFromBottom;
      this.keepFromBottom = null;
    }
    this.chat.setOlder(s?.older ? 'loading' : s?.paged && !s.more ? 'start' : null);
    // 一页不满一屏（滚不动、到不了顶）的接着读
    if (s?.more && !s.older) requestAnimationFrame(() => this.maybeOlder());
    this.syncJump?.();
    this.composer.setRunning(!!view.running);
    this.chat.setRunning(!!view.running);
    // 换了会话：重问核心这个会话里认哪些命令
    if (this.commandsFor !== this.current) {
      this.commandsFor = this.current;
      this.refreshCommands();
    }
    // 对话区画了一次：照它画的软件包（运行状态行这类）听这个事件；是状态事件，晚起来的包先拿到最后一份。
    // 带条目和会话状态（`entries`、`status`）：包照它们画（蓝图「照条目画」第 3 条）
    this.ctx.publish('view.changed', { session: s?.id ?? null, running: view.running, queued: view.queued, todos: s?.todos ?? [], todosDone: s?.todosDone ?? null,
      entries: s?.view.list ?? [], status: s?.view.status ?? null });
    // 排着的话（打断时有排着的不撤那一轮）
    this.queuedNow = view.queued ?? [];
    const f = statusFooter(s?.view.status ?? null, s?.limits ?? {});
    // 框下面的模型（蓝图「换模型的菜单」第 1 条）：新会话、选过还没生效的、用着池的照引用写；别的照核心报的模型、端点
    const ref = this.modelRef();
    const picked = this.current ? this.picked.get(this.current) : null;
    if (ref && (!this.current || picked || ref.startsWith('@'))) f.left = { ...f.left, ...footerOf(ref) };
    // 写给人看的：显示名、供应商的显示名（重名的、找不到的照模型名）
    f.left = { ...f.left, ...footerLabel({ model: f.left.model ?? null, endpoint: f.left.endpoint ?? null }, this.models) };
    // 思考强度：默认的不写（蓝图「换模型的菜单」第 1 条）
    const effort = this.effortLevel();
    f.left = { ...f.left, effort: effort === null ? null : effortLabel(effort) };
    const level = this.current ? null : this.pendingLevel;
    if (level) f.left = { ...f.left, level, label: levelLabel(level) };
    this.composer.drawFooter(f);
    this.artifacts.update(s ? artifactEvents(s.view.list, this.workdir()) : [], this.chat.where);
    this.followKids();
  }
}

/**
 * 回顾没有新内容（`cached`）的：照回应再画一次，排在要的那一刻最后一条条目后面。
 * @param {{items: any[]}} view @param {{after: any, text: string}[]} again
 */
function withLocalRecaps(view, again) {
  if (!again.length) return view;
  const items = [...view.items];
  again.forEach((r, i) => {
    const note = { type: 'note', key: `r-${r.after}-${i}`, seq: 0, turn: null, tone: 'dim', mark: null, text: '', recap: r.text, detail: null };
    const at = items.findIndex((it) => it.key === r.after);
    items.splice(at < 0 ? items.length : at + 1, 0, note);
  });
  return { ...view, items };
}

