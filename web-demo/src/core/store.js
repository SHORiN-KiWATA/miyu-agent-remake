// @ts-check
//! 头这边记着的会话：会话表（核心的会话列表流，`session-index.js`）、读进来的会话的持久事件、在收的那一次回复、限额、没看过的。
//! 不是每个会话都读日志：正在看的、在跑的、派过子代理的（左栏要画它下面那棵树）才读、订阅（蓝图 `web.md`「连核心」第 3 条）。
//!
//! 持久事件照序号接上；瞬时的 `model.delta` 只用来画「正在写」：这一次回复落了盘（`message.assistant` 的
//! `seen` 对上了）就扔掉（`kernel/events.md`「瞬时事件」）。看着它流出来时顺手记下每一块什么时候开始、什么时候收全
//! （`marks`），落了盘的思考、工具照它写用时：事件里没有每一块的时刻（蓝图 `web.md`「时间线的数」第 4 条）。
//! 展开、滚到哪这些纯界面的状态不在这里（`04-核心协议.md` P3）。

import { res } from '../util/res.js';
import { summarize } from '../model/session.js';
import { SessionIndex } from './session-index.js';

/** @typedef {import('../model/timeline.js').Block} Block */
/** @typedef {{turn: number, seen: number, blocks: Block[]}} Live */
/** @typedef {{turn: number, attempt: number, limit: number, message: string}} Retry 出了错、等着重试（瞬时的 `status`） */
/**
 * @typedef {{seen: number, since: number, written: number, expected: number|null, done: {before: number, after: number}|null, note: number|null}} Compacting
 *   在压缩（瞬时的 `compaction.progress`）：压好了记下前后的用量（`compaction.done`），落了盘的那一条的序号（走满以前先不画）。
 *   提前压好、直接换上的（`prepared`，核心 6-11）没有进度那一行：前后的用量先放 `compactReady`，等落了盘的那一条 `context.compacted` 来了再记上
 * @typedef {{id: string, events: any[], live: Live|null, marks: Map<string, {start: number, end: number|null}>,
 *   limits: any, replaying?: boolean, retry: Retry|null, compacting: Compacting|null, compactStats: Map<number, {before: number, after: number}>,
 *   compactReady: {before: number, after: number}|null, todos: {content: string, status: string}[], todosDone: {content: string, status: string}[]|null,
 *   changes: {after: number, at: string, body: any}[], model: {ref?: string, endpoint?: string, model?: string, effort?: {level: string, from: string}}|null}} Session
 */

/** 一个刚知道、还没读的会话。 */
export function emptySession(id) {
  return /** @type {Session} */ ({ id, events: [], live: null, marks: new Map(), limits: {}, retry: null, compacting: null, compactStats: new Map(), compactReady: null, changes: [], model: null, todos: [], todosDone: null });
}

export class Store {
  /** @param {import('./connection.js').Connection} conn */
  constructor(conn) {
    this.conn = conn;
    /** @type {Map<string, Session>} */
    this.sessions = new Map();
    /** 会话表：全部会话，照核心推的跟着变 */
    this.index = new SessionIndex(conn);
    /** 全部会话那一页开的老会话：最近活动排不进左栏的也列上 */
    this.extra = /** @type {Set<string>} */ (new Set());
    /** 没看过的：别的会话一轮结束了（读进来的照 `turn.ended`，没读的照会话表里 `busy` 去掉了） */
    this.unread = /** @type {Set<string>} */ (new Set());
    /** 正在看的会话：别的会话一轮结束了才记成没看过。 */
    this.viewing = /** @type {string|null} */ (null);
    /** 别处删掉了一个读进来的会话（不是这一页删的）：界面收掉它 @type {(id: string) => void} */
    this.removed = () => {};
    this.listeners = new Set();
    conn.onPush((method, params) => this.push(method, params));
    this.index.on((id, before, after) => this.entryChanged(id, before, after));
  }

  /**
   * 左栏列哪些（蓝图 `web.md`「左栏」）：顶层的会话（不是一次性的、不是子会话）照最近活动的几个（`layout.json` 的 `listed_sessions`），
   * 加上置顶的、全部会话那一页开过的；先后由界面排（`rank`）。
   */
  get order() {
    const top = this.index.all().filter((e) => !e.oneshot && !e.parent);
    const at = (e) => Date.parse(e.last_active ?? '') || 0;
    const recent = [...top].sort((a, b) => at(b) - at(a)).slice(0, res.layout.listed_sessions).map((e) => e.session);
    const pinned = top.filter((e) => e.pinned).map((e) => e.session);
    return [...new Set([...recent, ...pinned, ...this.extra])];
  }

  /**
   * 会话表里一项变了：一轮结束了（`busy` 去掉）、你没在看的记成没看过；开始跑的顶层会话读进来（在跑的才有时间线、子代理、后台命令）；
   * 别处删掉了读进来的、不是这一页删的，交给界面收掉。
   */
  entryChanged(id, before, after) {
    if (before?.busy && !after?.busy && id !== this.viewing) this.unread.add(id);
    if (after?.busy && !after.parent && !after.oneshot && !this.sessions.has(id)) this.follow(id);
    const s = this.sessions.get(id);
    if (!after) {
      this.unread.delete(id);
      this.extra.delete(id);
      if (s && !s.gone) this.removed(id);
    }
    this.changed();
  }

  /** 读进来一个会话，读不了的不留（原因记在控制台）。 */
  follow(id) {
    this.load(id).then(() => this.changed(), (err) => {
      console.error(`会话 ${id} 读不了：${err.message}`);
      this.sessions.delete(id);
      this.changed();
    });
  }

  /** 有变化就告诉界面。 */
  on(fn) { this.listeners.add(fn); }

  changed() { for (const fn of this.listeners) fn(); }

  /**
   * 起来：订阅会话表（在跑的会话由 `entryChanged` 读进来）；左栏里派过子代理的也读进来，好画它下面那棵树（蓝图 `web.md`「连核心」第 3 条）。
   *
   * # Errors
   * 核心拒绝列会话时抛出来；单个会话读不了的跳过。
   */
  async boot() {
    await this.index.start();
    this.followParents();
    this.changed();
  }

  /** 左栏里有子会话的（会话表里有谁的 `parent` 是它）还没读的，读进来。 */
  followParents() {
    const parents = new Set(this.index.all().map((e) => e.parent).filter(Boolean));
    for (const id of this.order) if (parents.has(id) && !this.sessions.has(id)) this.follow(id);
  }

  /**
   * 订阅一个会话、读它的历史：订阅带 `after: 0`，核心先把整份日志照原样补推过来（普通的 `event` 推送，都在回应前面）、再接着推新的
   * （施工 3-8 六补）；推来的照序号接上、去重（`persisted`）。补历史的那一段不记「没看过」。`listed` 为假的（子代理的会话）不进
   * 会话表的顶层。
   */
  async load(id) {
    const s = emptySession(id);
    this.sessions.set(id, s);
    await this.subscribe(s, 0);
  }

  /** 订阅（带 `after`）：补推的是历史，不记「没看过」；回应里的限额记下。 */
  async subscribe(s, after) {
    s.replaying = true;
    try {
      const { limits, model, todos } = await this.conn.request('subscribe', { session: s.id, stream: 'events', after });
      s.limits = limits ?? s.limits ?? {};
      // 会话接下来请求的模型（核心施工 8-10）：框下面那一行照它写
      s.model = model ?? null;
      // 待办（核心 D-3）：清单不空时回应里带着；之后照瞬时的 `todos.changed` 换
      s.todos = todos ?? [];
      s.todosDone = null;
    } finally {
      s.replaying = false;
    }
  }

  /**
   * 开一个会话，排在最前面（蓝图 `web.md`「连核心」第 5 条：第一句话发出去时才开）。`model` 是还没开时在换模型的菜单里选的引用。
   *
   * # Errors
   * 核心拒绝时抛出来。
   */
  async create(cwd, model = null, persona = null) {
    // 还没开的新会话里选过模型（核心施工 8-8）、选过人格（P-1）的，开的时候带上；人格不写的照配置项 `persona.default`
    const { session } = await this.conn.request('session.create', { cwd, ...(model ? { model } : {}), ...(persona ? { persona } : {}) });
    // 推送还没到时先记上，左栏当场有它
    this.index.seed(session, { cwd });
    await this.load(session);
    this.changed();
    return session;
  }

  /** 要删这个会话了（`on` 为 false 是没删成）：这期间推来的 `resync` 不去补（它是因为删才停的）。 */
  leaving(id, on = true) {
    const s = this.sessions.get(id);
    if (s) s.gone = on;
  }

  /** 删了的会话：从表里拿掉，不再列。 */
  drop(id) {
    this.sessions.delete(id);
    this.extra.delete(id);
    this.unread.delete(id);
    this.changed();
  }

  /** 说一句话；`extra` 是跟着发的（附件 `{attachments}`、在哪个目录干活 `{cwd}`），合进参数；交回核心的回应（`cwd` 是实际在哪干活）。 */
  send(id, text, extra = {}) { return this.conn.request('session.send', { session: id, text, ...extra }); }

  /**
   * 左栏的一项：读进来了的照日志（每条事件都跟着走），没读的照会话表（标题、没标题的拿第一句话的预览、在不在跑、置顶、最近活动）。
   */
  summary(id) {
    const s = this.sessions.get(id);
    const e = this.index.get(id);
    const log = summarize(id, s?.events ?? []);
    const read = !!s?.events.length;
    const listed = Date.parse(e?.last_active ?? '');
    const active = Math.max(log.active ?? 0, Number.isNaN(listed) ? 0 : listed) || null;
    return read
      ? { ...log, active, unread: this.unread.has(id) }
      : { ...log, title: e?.title ?? e?.preview ?? null, running: !!e?.busy, pinned: !!e?.pinned, active, unread: this.unread.has(id) };
  }

  /**
   * 一个还没读的会话：读进来、订阅上。子代理的（挂在派它的会话下面）不进会话表的顶层；`listed` 的（全部会话那一页开的老会话）
   * 进，排在最后。读过的不再读。
   */
  async ensure(id, listed = false) {
    if (listed) this.extra.add(id);
    if (this.sessions.has(id)) {
      if (listed) this.changed();
      return;
    }
    await this.load(id);
    this.changed();
  }

  /** 压好了、进度条走满了：进度那一行收掉，落了盘的那一行露出来。 @param {string} id */
  finishCompaction(id) {
    const s = this.sessions.get(id);
    if (!s?.compacting) return;
    s.compacting = null;
    this.changed();
  }

  /** 看这个会话：没看过的记号去掉。 */
  view(id) {
    this.viewing = id;
    if (id) this.unread.delete(id);
    this.changed();
  }

  push(method, p) {
    const s = p?.session ? this.sessions.get(p.session) : null;
    if (!s) return;
    if (method === 'resync') {
      if (s.gone) return;
      this.catchUp(s).catch((err) => console.error(`会话 ${s.id} 掉队以后补不上：${err.message}`));
      return;
    }
    if (method !== 'event') return;
    const e = p.event;
    if (e.seq != null) this.persisted(s, e);
    else this.transient(s, e);
    this.changed();
  }

  /**
   * 断了又连上了（蓝图 `web.md`「连核心」第 1 条）：读进来了的会话一个个重新订阅、补上漏掉的（和掉了队一样）；会话表重新订阅，
   * 断着时别处开的、改的、删的照它跟上。单个会话补不上的跳过，原因记在控制台。
   */
  async resume() {
    for (const s of this.sessions.values()) {
      if (s.gone) continue;
      await this.catchUp(s).catch((err) => console.error(`会话 ${s.id} 重连以后补不上：${err.message}`));
    }
    await this.index.start();
    this.followParents();
    this.changed();
  }

  /** 掉了队、断线重连：带上最后看到的序号重新订阅，核心补上漏掉的（`04-核心协议.md` 第七节，施工 3-8 六补）。 */
  async catchUp(s) {
    // 带上最后看到的序号重新订阅，核心补上漏掉的（施工 3-8 六补）
    await this.subscribe(s, s.events.at(-1)?.seq ?? 0);
    this.changed();
  }

  persisted(s, e) {
    if (s.events.length && s.events.at(-1).seq >= e.seq) return;
    s.events.push(e);
    // 回复落了盘、一轮结束：在收的扔掉，还开着的块停在这一刻
    if (e.kind === 'message.assistant' && s.live?.seen === e.body.seen) {
      closeAll(s.live, Date.parse(e.at));
      s.live = null;
    }
    // 压缩（蓝图 `web.md`「压缩的进度」）：压好了、落了盘的那一条记上前后的用量，走满以前先不画；没压成的、这一轮先结束了的
    // 进度那一行收掉
    if (e.kind === 'context.compacted' && s.compacting?.done) {
      s.compactStats.set(e.seq, s.compacting.done);
      s.compacting.note = e.seq;
    }
    if (e.kind === 'context.compacted' && !s.compacting && s.compactReady) {
      if (e.body.trigger !== 'clear') s.compactStats.set(e.seq, s.compactReady);
      s.compactReady = null;
    }
    if (e.kind === 'model.called' && e.body.compaction && e.body.result === 'error') s.compacting = null;
    if (e.kind === 'turn.ended' && s.compacting && !s.compacting.done) s.compacting = null;
    if (e.kind === 'turn.ended') {
      s.compactReady = null;
      if (s.live) closeAll(s.live, Date.parse(e.at));
      s.live = null;
      s.retry = null;
      if (s.id !== this.viewing && !s.replaying) this.unread.add(s.id);
    }
  }

  /**
   * 瞬时的增量攒成在收的那一次回复（`kernel/events.md`）：`start` 开一块（调工具的带工具名），`text` 往里接，`end` 这一块
   * 收全；同一次请求的下一块开始了，前面的也算收全（`tui.md`「时间线」第 10 条）。每一块开始、收全的时刻照事件的 `at`
   * 记进 `marks`，编号是 `请求:种类:这次请求里第几块这一种`。`tool.progress` 不画（照 TUI，输出等结果来了才有）。
   *
   * `status` 带着 `retry` 的是出了错、等着重试：记下来，运行状态行接着写「重试 1/5：原话」；下一段 `model.delta`
   * 来了、这一轮结束了就去掉（蓝图 `web.md`「运行状态行」、`kernel/events.md` 瞬时事件第 17 条）。
   */
  transient(s, e) {
    // 待办换了（核心 D-3，瞬时的 `todos.changed`）：整份换上，空的是清空了
    if (e.kind === 'todos.changed') {
      s.todos = e.body?.todos ?? [];
      // 因为全做完了清空的带着清空前那一份（核心 D-3 补）：待办那一块照它停一下再收
      s.todosDone = s.todos.length ? null : e.body?.done ?? null;
      return;
    }
    // 压缩的进度：写了多少、估计多少；压好了记下前后的用量，等界面走满了再收（`finishCompaction`）。写了的变少了是重来
    // （提前压好的那次在线上失败了，换成当场的摘要请求，核心 6-11），从头走
    if (e.kind === 'compaction.progress') {
      const b = e.body;
      const restart = (b.written ?? 0) < (s.compacting?.written ?? 0);
      if (!s.compacting || s.compacting.seen !== b.seen || s.compacting.done || restart) {
        s.compacting = { seen: b.seen, since: Date.parse(e.at), written: 0, expected: b.expected ?? null, done: null, note: null };
      }
      s.compacting.written = b.written ?? s.compacting.written;
      if (b.expected) s.compacting.expected = b.expected;
      return;
    }
    if (e.kind === 'compaction.done') {
      const done = { before: e.body.before, after: e.body.after };
      // 落了盘的那一条先到了的（核心「同时」推，两条谁先到不一定）：补记上前后的用量
      const last = s.events.at(-1);
      const landed = last?.kind === 'context.compacted' && last.body.trigger !== 'clear' && !s.compactStats.has(last.seq);
      // 提前压好、直接换上的（`prepared`，核心 6-11）前面没有进度那一行：只记用量，不出进度、不走满
      if (!s.compacting) {
        if (landed) s.compactStats.set(last.seq, done);
        else s.compactReady = done;
        return;
      }
      s.compacting.done = done;
      if (landed) {
        s.compactStats.set(last.seq, done);
        s.compacting.note = last.seq;
      }
      return;
    }
    if (e.kind === 'status' && e.body?.retry && typeof e.body.retry === 'object') {
      const r = e.body.retry;
      // 什么时候重试：这条事件的时刻加上 `wait_ms`（运行状态行倒数）
      const due = typeof r.wait_ms === 'number' ? Date.parse(e.at) + r.wait_ms : undefined;
      s.retry = { turn: e.turn, attempt: r.attempt, limit: r.limit, message: r.message ?? '', failover: r.failover === true, ...(due === undefined || Number.isNaN(due) ? {} : { due }) };
      return;
    }
    // 出错换了模型（核心施工 8-9）：限额跟着换（框下面那一行的窗口）；换模型的记下来，时间线上出一行（`withChanges`）
    if (e.kind === 'model.changed') {
      if (e.body?.limits) s.limits = { ...s.limits, ...e.body.limits };
      // 接下来请求的模型：轮换的池只有 `ref`（8-10）
      s.model = { ref: e.body?.ref, ...(e.body?.endpoint ? { endpoint: e.body.endpoint } : {}), ...(e.body?.model ? { model: e.body.model } : {}),
        // 接下来请求那个模型的思考强度（`{level, from}`，核心施工 8-18；什么都不带的不写）
        ...(e.body?.effort ? { effort: e.body.effort } : {}) };
      if (e.body?.why === 'failover') s.changes.push({ after: s.events.at(-1)?.seq ?? 0, at: e.at, body: e.body });
      return;
    }
    if (e.kind !== 'model.delta') return;
    s.retry = null;
    const b = e.body;
    const at = Date.parse(e.at);
    if (!s.live || s.live.seen !== b.seen) s.live = { turn: e.turn, seen: b.seen, blocks: [] };
    const live = s.live;
    if (b.start) {
      closeAll(live, at);
      const nth = live.blocks.filter((x, i) => x && x.kind === b.start && i !== b.index).length;
      const mark = { start: at, end: /** @type {number|null} */ (null) };
      s.marks.set(`${b.seen}:${b.start}:${nth}`, mark);
      live.blocks[b.index] = { kind: b.start, name: b.name, text: '', done: false, start: at, end: null, mark };
    }
    const block = live.blocks[b.index];
    if (!block) return;
    if (b.text) block.text += b.text;
    if (b.end) close(block, at);
  }
}

/** 一块收全了：停表，记下的时刻跟着停。收过的不再动。 */
function close(block, at) {
  if (block.done) return;
  block.done = true;
  block.end = at;
  if (block.mark) block.mark.end = at;
}

/** 这次请求里还开着的块都算收全。 */
function closeAll(live, at) {
  for (const block of live.blocks) if (block) close(block, at);
}
