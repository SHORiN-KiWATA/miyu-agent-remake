// @ts-check
//! 挂载位（蓝图 `web/architecture.md`「挂载位」，照 deepseek-harness 的 slots）：页面上每一块挂在某个挂载位里，由声明它的包
//! 画出挂的地方。这里只管表，不碰 DOM：谁挂了什么、照什么排、谁在最上面；画由声明的包照表画，`Slots.draw` 兜着每一件的错。
//!
//! - `single` 独占：新挂的盖住原来的，拿掉了原来的露出来；
//! - `list` 一串：照 `order` 排（一样的照 `id`），同一个 `id` 后来的盖住先来的；
//! - `keyed` 按键分派：照键找，找不到用兜底（键是 `*`）。
//!
//! 往没声明的挂载位里挂是 bug，报错；读没声明的（比如刚撤回，看着它的正在重画）是空的。挂别的包声明的挂载位用 `mount`：
//! 还没声明就等着，声明了挂上，撤回了跟着没，再声明又挂上，不看加载的先后（照 deepseek-harness 的 `slots.inject`）。
//!
//! 当服务用时按包绑一份（`bind`）：经那个包声明、挂的，跟着它撤回。

/**
 * @typedef {{id: string, order?: number, key?: string, render: (...args: any[]) => any}} Entry 挂的一件：画法由声明的包约定
 * @typedef {Entry & {owner: string, seq: number}} Mounted
 * @typedef {'single'|'list'|'keyed'} Kind
 */

const KINDS = ['single', 'list', 'keyed'];

export class Slots {
  constructor() {
    /** @type {Map<string, {kind: Kind, owner: string, entries: Mounted[]}>} */
    this.table = new Map();
    /** @type {Map<string, Set<() => void>>} */
    this.watchers = new Map();
    /** 挂的先后：同一个 id、独占的，后来的在上面 */
    this.seq = 0;
  }

  /**
   * 声明一个挂载位；交回怎么撤回（连同里面挂的一起收掉）。
   * @param {string} name `地方.东西`
   * @param {Kind} kind
   * @param {string} owner 声明它的包
   */
  declare(name, kind, owner) {
    if (!KINDS.includes(kind)) throw new Error(`挂载位 ${name} 的种类 ${kind} 不认识（只有 ${KINDS.join('、')}）`);
    if (this.table.has(name)) throw new Error(`挂载位 ${name} 已经由 ${this.table.get(name)?.owner} 声明了`);
    const slot = { kind, owner, entries: /** @type {Mounted[]} */ ([]) };
    this.table.set(name, slot);
    this.changed(name);
    return () => {
      if (this.table.get(name) !== slot) return;
      this.table.delete(name);
      this.changed(name);
    };
  }

  /**
   * 往挂载位里挂一件；交回怎么拿下来。
   * @param {string} name
   * @param {Entry} entry
   * @param {string} owner 挂它的包
   */
  register(name, entry, owner) {
    const slot = this.slot(name);
    const mounted = { ...entry, owner, seq: ++this.seq };
    slot.entries.push(mounted);
    this.changed(name);
    return () => {
      const i = slot.entries.indexOf(mounted);
      if (i < 0) return;
      slot.entries.splice(i, 1);
      if (this.table.get(name) === slot) this.changed(name);
    };
  }

  /**
   * 挂到一个可能还没声明的挂载位：声明了挂上，撤回了跟着没（挂的随声明一起收掉），再声明又挂上；交回怎么不挂了。
   * @param {string} name
   * @param {Entry} entry
   * @param {string} owner
   */
  mount(name, entry, owner) {
    /** @type {{slot: any, undo: () => void}|null} */
    let on = null;
    let busy = false;
    const sync = () => {
      if (busy) return;
      busy = true;
      try {
        const slot = this.table.get(name);
        if (slot && on?.slot !== slot) on = { slot, undo: this.register(name, entry, owner) };
        else if (!slot) on = null;
      } finally {
        busy = false;
      }
    };
    const stop = this.watch(name, sync);
    sync();
    return () => {
      stop();
      on?.undo();
      on = null;
    };
  }

  /** 一串：照 `order` 排，一样的照 `id`；同一个 `id` 只留最后挂的。 */
  list(name) {
    const latest = new Map();
    for (const e of this.table.get(name)?.entries ?? []) if ((latest.get(e.id)?.seq ?? -1) < e.seq) latest.set(e.id, e);
    return [...latest.values()].sort((a, b) => (a.order ?? 0) - (b.order ?? 0) || a.id.localeCompare(b.id));
  }

  /** 独占：最后挂的那一件；空着是 `null`。 */
  single(name) {
    return (this.table.get(name)?.entries ?? []).reduce((top, e) => (!top || e.seq > top.seq ? e : top), /** @type {Mounted|null} */ (null));
  }

  /** 按键分派：这个键最后挂的那一件，没有用兜底（`*`），都没有是 `null`。 */
  pick(name, key) {
    const entries = this.table.get(name)?.entries ?? [];
    const latest = (k) => entries.filter((e) => e.key === k).reduce((top, e) => (!top || e.seq > top.seq ? e : top), /** @type {Mounted|null} */ (null));
    return latest(key) ?? latest('*');
  }

  /** 看着一个挂载位：挂的、拿的、声明、撤回都通知；交回怎么不看。 */
  watch(name, fn) {
    const set = this.watchers.get(name) ?? new Set();
    this.watchers.set(name, set);
    set.add(fn);
    return () => set.delete(fn);
  }

  /** 按包绑一份（当服务用时由上下文调）：声明、挂都走那个包的 `effect`，跟着它撤回。 */
  bind(ctx) {
    return {
      declare: (name, kind) => ctx.effect(() => this.declare(name, kind, ctx.id)),
      register: (name, entry) => ctx.effect(() => this.register(name, entry, ctx.id)),
      mount: (name, entry) => ctx.effect(() => this.mount(name, entry, ctx.id)),
      list: (name) => this.list(name),
      single: (name) => this.single(name),
      pick: (name, key) => this.pick(name, key),
      watch: (name, fn) => ctx.effect(() => this.watch(name, fn)),
      draw: Slots.draw,
    };
  }

  /**
   * 画一件，兜着它的错：抛错的交回 `{failed, owner, reason}`，声明的包在那一格画「这一块出错了」，别的照常。
   * @param {Mounted} entry
   */
  static draw(entry, ...args) {
    try {
      return entry.render(...args);
    } catch (err) {
      console.error(`${entry.owner} 挂的 ${entry.id} 画的时候出错了`, err);
      return { failed: true, owner: entry.owner, reason: err instanceof Error ? err.message : String(err) };
    }
  }

  slot(name) {
    const slot = this.table.get(name);
    if (!slot) throw new Error(`挂载位 ${name} 还没有人声明`);
    return slot;
  }

  changed(name) {
    for (const fn of [...(this.watchers.get(name) ?? [])]) fn();
  }
}
