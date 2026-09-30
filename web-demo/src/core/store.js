// @ts-check
//! 头这边记着的会话：会话表、每个会话的持久事件、在收的那一次回复、限额、没看过的。
//!
//! 持久事件照序号接上；瞬时的 `model.delta` 只用来画「正在写」：这一次回复落了盘（`message.assistant` 的
//! `seen` 对上了）就扔掉（`kernel/events.md`「瞬时事件」）。看着它流出来时顺手记下每一块什么时候开始、什么时候收全
//! （`marks`），落了盘的思考、工具照它写用时：事件里没有每一块的时刻（蓝图 `web.md`「时间线的数」第 4 条）。
//! 展开、滚到哪这些纯界面的状态不在这里（`04-核心协议.md` P3）。

import { res } from '../util/res.js';
import { summarize } from '../model/session.js';

/** @typedef {import('../model/timeline.js').Block} Block */
/** @typedef {{turn: number, seen: number, blocks: Block[]}} Live */
/** @typedef {{turn: number, attempt: number, limit: number, message: string}} Retry 出了错、等着重试（瞬时的 `status`） */
/**
 * @typedef {{id: string, events: any[], live: Live|null, marks: Map<string, {start: number, end: number|null}>,
 *   limits: any, unread: boolean, retry: Retry|null}} Session
 */

/** 一个刚知道、还没读的会话。 */
export function emptySession(id) {
  return /** @type {Session} */ ({ id, events: [], live: null, marks: new Map(), limits: {}, unread: false, retry: null });
}

export class Store {
  /** @param {import('./connection.js').Connection} conn */
  constructor(conn) {
    this.conn = conn;
    /** @type {Map<string, Session>} */
    this.sessions = new Map();
    /** 会话的先后：新的在前（`session.list` 的先后，蓝图 `protocol.md`）。 */
    this.order = /** @type {string[]} */ ([]);
    /** 正在看的会话：别的会话一轮结束了才记成没看过。 */
    this.viewing = /** @type {string|null} */ (null);
    this.listeners = new Set();
    conn.onPush((method, params) => this.push(method, params));
  }

  /** 有变化就告诉界面。 */
  on(fn) { this.listeners.add(fn); }

  changed() { for (const fn of this.listeners) fn(); }

  /**
   * 起来：列出会话，最新的几个（`layout.json` 的 `listed_sessions`）读日志、订阅（蓝图 `web.md`「连核心」第 3 条）。
   *
   * # Errors
   * 核心拒绝列会话时抛出来；单个会话读不了的跳过。
   */
  async boot() {
    const { sessions } = await this.conn.request('session.list', {});
    // 一次性的（`miyu ask`）、子会话（`parent` 不是空的，子代理的）不列（蓝图 `web.md`「会话表的一项」）
    const ids = sessions.filter((s) => !s.oneshot && !s.parent).slice(0, res.layout.listed_sessions).map((s) => s.session);
    for (const id of ids) {
      try {
        await this.load(id);
      } catch (err) {
        // 读不了的会话不列，原因记在控制台
        console.error(`会话 ${id} 读不了：${err.message}`);
        this.sessions.delete(id);
        this.order = this.order.filter((x) => x !== id);
      }
    }
    this.changed();
  }

  /**
   * 订阅一个会话、读它的日志：先订阅再读，中间落盘的两边都可能有，按序号去重。`listed` 为假的（子代理的会话）不进会话表的顶层。
   */
  async load(id, listed = true) {
    const s = emptySession(id);
    this.sessions.set(id, s);
    if (listed && !this.order.includes(id)) this.order.push(id);
    const { limits } = await this.conn.request('subscribe', { session: id, stream: 'events' });
    s.limits = limits ?? {};
    const { events } = await this.conn.request('events.read', { session: id });
    const known = new Set(s.events.map((e) => e.seq));
    s.events = [...events.filter((e) => !known.has(e.seq)), ...s.events].sort((a, b) => a.seq - b.seq);
  }

  /**
   * 开一个会话，排在最前面（蓝图 `web.md`「连核心」第 5 条：第一句话发出去时才开）。
   *
   * # Errors
   * 核心拒绝时抛出来。
   */
  async create(cwd) {
    const { session } = await this.conn.request('session.create', { cwd });
    this.order = [session, ...this.order.filter((x) => x !== session)];
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
    this.order = this.order.filter((x) => x !== id);
    this.changed();
  }

  /** 说一句话；`extra` 是跟着发的（附件：`{attachments}`），合进参数。 */
  send(id, text, extra = {}) { return this.conn.request('session.send', { session: id, text, ...extra }); }

  /** 左栏的一项。 */
  summary(id) {
    const s = this.sessions.get(id);
    return { ...summarize(id, s?.events ?? []), unread: !!s?.unread };
  }

  /**
   * 一个还没读的会话：读进来、订阅上。子代理的（挂在派它的会话下面）不进会话表的顶层；`listed` 的（全部会话那一页开的老会话）
   * 进，排在最后。读过的不再读。
   */
  async ensure(id, listed = false) {
    if (listed && !this.order.includes(id)) this.order.push(id);
    if (this.sessions.has(id)) {
      if (listed) this.changed();
      return;
    }
    await this.load(id, listed);
    this.changed();
  }

  /** 看这个会话：没看过的记号去掉。 */
  view(id) {
    this.viewing = id;
    const s = id ? this.sessions.get(id) : null;
    if (s) s.unread = false;
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
   * 断了又连上了（蓝图 `web.md`「连核心」第 1 条）：读进来了的会话一个个重新订阅、补上漏掉的（和掉了队一样）；再列一遍会话，
   * 断着时别处开的新会话（顶层的）读进来、排在最前面。单个会话补不上的跳过，原因记在控制台。
   */
  async resume() {
    for (const s of this.sessions.values()) {
      if (s.gone) continue;
      await this.catchUp(s).catch((err) => console.error(`会话 ${s.id} 重连以后补不上：${err.message}`));
    }
    const { sessions } = await this.conn.request('session.list', {});
    const fresh = sessions.filter((x) => !x.oneshot && !x.parent && !this.sessions.has(x.session)).map((x) => x.session);
    for (const id of fresh) await this.load(id, false).catch((err) => console.error(`会话 ${id} 读不了：${err.message}`));
    this.order = [...fresh.filter((id) => this.sessions.has(id)), ...this.order];
    this.changed();
  }

  /** 掉了队：重新订阅，再把漏掉的取回来（`04-核心协议.md` 第七节）。 */
  async catchUp(s) {
    await this.conn.request('subscribe', { session: s.id, stream: 'events' });
    const after = s.events.at(-1)?.seq ?? 0;
    const { events } = await this.conn.request('events.read', { session: s.id, after });
    for (const e of events) this.persisted(s, e);
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
    if (e.kind === 'turn.ended') {
      if (s.live) closeAll(s.live, Date.parse(e.at));
      s.live = null;
      s.retry = null;
      if (s.id !== this.viewing) s.unread = true;
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
    if (e.kind === 'status' && e.body?.retry && typeof e.body.retry === 'object') {
      const r = e.body.retry;
      s.retry = { turn: e.turn, attempt: r.attempt, limit: r.limit, message: r.message ?? '' };
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
