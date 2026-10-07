// @ts-check
//! 会话表（核心施工 9-5 的会话列表流，蓝图 `protocol.md`「会话列表的推送」）：`subscribe {stream: "sessions"}` 拿到整张表（一次性的、
//! 子会话都在），之后照 `sessions.changed` 整项换、删掉；掉了队（`resync`）重新订阅。左栏、全部会话那一页、`/sessions` 照它列，
//! 不用先读每个会话的日志（蓝图 `web.md`「连核心」第 3 条）。旧核心没有这个流的，退回 `session.list` 读一次，不跟着变。

/**
 * 表里的一项，和 `session.list` 的一样：有的才写。
 * @typedef {{session: string, busy?: boolean, cwd?: string, last_active?: string, oneshot?: boolean, parent?: string|null,
 *   pinned?: boolean, title?: string, preview?: string}} Entry
 */

/** 一项变了：之前的、之后的（删掉的是 `null`，新来的之前是 `null`）。 @typedef {(id: string, before: Entry|null, after: Entry|null) => void} Change */

export class SessionIndex {
  /** @param {{request: (method: string, params: any) => Promise<any>, onPush: (fn: (method: string, params: any) => void) => void}} conn */
  constructor(conn) {
    this.conn = conn;
    /** @type {Map<string, Entry>} */
    this.entries = new Map();
    /** @type {Set<Change>} */
    this.listeners = new Set();
    /** 订阅着（核心有这个流）；退回读一次的是假 */
    this.live = false;
    conn.onPush((method, params) => this.push(method, params));
  }

  /** @param {Change} fn */
  on(fn) { this.listeners.add(fn); }

  /** @param {string} id */
  get(id) { return this.entries.get(id) ?? null; }

  /** 全部，照核心给的先后（从新到旧）。 */
  all() { return [...this.entries.values()]; }

  /**
   * 订阅、拿到整张表（起来、掉了队、断线重连都走这里）：和手里的比，变了的一项项告诉听的。
   *
   * # Errors
   * 订阅、退回去读一次都被拒时抛出来。
   */
  async start() {
    let sessions;
    try {
      ({ sessions } = await this.conn.request('subscribe', { stream: 'sessions' }));
      this.live = true;
    } catch (err) {
      // 旧核心没有这个流（`stream` 只认 `events`、`config`）：读一次
      console.error(`订阅不了会话列表，读一次：${/** @type {any} */ (err)?.message}`);
      ({ sessions } = await this.conn.request('session.list', {}));
      this.live = false;
    }
    this.replace(sessions ?? []);
  }

  /** 整张表换上：没了的算删掉。 @param {Entry[]} list */
  replace(list) {
    const next = new Map(list.map((e) => [e.session, e]));
    const before = this.entries;
    this.entries = next;
    for (const [id, old] of before) if (!next.has(id)) this.emit(id, old, null);
    for (const [id, e] of next) if (!same(before.get(id), e)) this.emit(id, before.get(id) ?? null, e);
  }

  /**
   * 自己刚开的会话：推送还没到时先记上一项，左栏当场有它（推送到了整项换掉）。
   * @param {string} id @param {Partial<Entry>} [fields]
   */
  seed(id, fields = {}) {
    if (this.entries.has(id)) return;
    const e = { session: id, parent: null, oneshot: false, last_active: new Date().toISOString(), ...fields };
    // 新的在前，照核心的先后
    this.entries = new Map([[id, e], ...this.entries]);
    this.emit(id, null, e);
  }

  push(method, p) {
    if (method === 'resync' && p?.stream === 'sessions') {
      this.start().catch((err) => console.error(`会话列表掉队以后补不上：${err.message}`));
      return;
    }
    if (method !== 'sessions.changed' || !p?.session) return;
    const before = this.entries.get(p.session) ?? null;
    if (p.removed) {
      if (!before) return;
      this.entries.delete(p.session);
      this.emit(p.session, before, null);
      return;
    }
    if (!p.entry) return;
    if (before) this.entries.set(p.session, p.entry);
    else this.entries = new Map([[p.session, p.entry], ...this.entries]);
    this.emit(p.session, before, p.entry);
  }

  emit(id, before, after) { for (const fn of this.listeners) fn(id, before, after); }
}

/** 两项一样（格照字母先后写出来比）。 */
function same(a, b) {
  if (!a || !b) return a === b;
  const text = (x) => JSON.stringify(Object.keys(x).sort().map((k) => [k, x[k]]));
  return text(a) === text(b);
}
