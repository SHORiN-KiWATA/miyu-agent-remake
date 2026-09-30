// @ts-check
//! 上下文（蓝图 `web/architecture.md`「上下文（系统调用）」，照 Cordis 的论文）：服务表、事件，和一个包的一次加载（纤程）。
//!
//! - 撤回：包经 `ctx` 做的每件事都是一个效果，交回怎么撤回；停用、卸载、重装时照登记的反序撤回，每件只撤一次。
//! - 依赖：清单的 `inject` 写要哪些服务（带 `?` 的有就用、没有也行）。都在了才跑 `apply`（就绪），缺了等着；用着的服务
//!   换了提供者、拿掉了，这个包撤回重来。带 `~` 的是用的时候再找（总是现在的那个），来了、换了、没了都不让这个包重来：
//!   给大的包用（整页不该因为灯箱装上、停了就整个重来）。没写的服务碰了报错，算这个包故障（能力检查）。
//! - 故障只算它自己：`apply` 抛错，做到一半的撤回，别的包照常。

import { lookup } from '../lib/text.js';

/** 界面语言：包的字照它取（以后跟着配置走）。 */
const LANGUAGE = 'zh-CN';

/** 服务表和事件：整个页面一份。 */
export class Registry {
  constructor() {
    /** @type {Map<string, {value: any, owner: Fiber}>} */
    this.services = new Map();
    /** @type {Map<string, Set<Function>>} */
    this.listeners = new Map();
    /** @type {Set<Fiber>} 活着的纤程：服务变了照它们要的通知 */
    this.fibers = new Set();
    /** @type {Fiber[]} 等着重新对一遍依赖的 */
    this.queue = [];
    this.flushing = false;
  }

  has(name) { return this.services.has(name); }

  get(name) { return this.services.get(name)?.value; }

  /** 提供一个服务：一个名字只能有一个提供者（同时好几个提供者的是职能，见 `seams.js`）。 */
  set(name, value, owner) {
    const had = this.services.get(name);
    if (had && had.owner !== owner) throw new Error(`服务 ${name} 已经由 ${had.owner.manifest.id} 提供`);
    this.services.set(name, { value, owner });
    this.changed(name);
  }

  /** 拿掉一个服务（只拿掉自己提供的）。 */
  unset(name, owner) {
    if (this.services.get(name)?.owner !== owner) return;
    this.services.delete(name);
    this.changed(name);
  }

  /** 服务变了：要它的纤程排队重新对一遍依赖。对的时候又变了的，接着排，一个个来，不重入。 */
  changed(name) {
    for (const fiber of this.fibers) if (fiber.needs(name) && !this.queue.includes(fiber)) this.queue.push(fiber);
    if (this.flushing) return;
    this.flushing = true;
    try {
      while (this.queue.length) this.queue.shift()?.refresh();
    } finally {
      this.flushing = false;
    }
  }

  /** 听一个事件；交回怎么不听。 */
  on(event, fn) {
    const set = this.listeners.get(event) ?? new Set();
    this.listeners.set(event, set);
    set.add(fn);
    return () => set.delete(fn);
  }

  /** 发一个事件：一个听的抛错不耽误别的，记在控制台。 */
  emit(event, ...args) {
    for (const fn of [...(this.listeners.get(event) ?? [])]) {
      try {
        fn(...args);
      } catch (err) {
        console.error(`事件 ${event} 的一个处理出错了`, err);
      }
    }
  }
}

/**
 * @typedef {{id: string, inject?: string[], settings?: Record<string, any>, text?: Record<string, any>}} Manifest
 * @typedef {{manifest: Manifest, apply: (ctx: any) => void}} Package 一个包：清单加入口
 * @typedef {'idle'|'pending'|'active'|'failed'|'disposed'} State 还没起、等着（缺服务）、就绪、故障、撤回了
 */

/** 一个包的一次加载。 */
export class Fiber {
  /**
   * @param {Registry} registry
   * @param {Package} pkg
   * @param {Record<string, any>} config 这个包的设置项的最终值（`config.js` 合好的）
   */
  constructor(registry, pkg, config) {
    this.registry = registry;
    this.manifest = pkg.manifest;
    this.apply = pkg.apply;
    this.config = Object.freeze({ ...config });
    /** @type {State} */
    this.state = 'idle';
    /** @type {string|null} 故障的原因 */
    this.reason = null;
    /** @type {Array<() => void>} 撤回，照登记的先后 */
    this.disposers = [];
    /** 就绪时用着的服务：变了就重来 */
    this.seen = new Map();
    /** 听设置项当场变的（`ctx.watchConfig`）：这一次加载登记的 */
    this.configWatchers = new Set();
    this.deps = (this.manifest.inject ?? []).map((n) => ({ name: n.replace(/[?~]$/, ''), optional: /[?~]$/.test(n), lazy: n.endsWith('~') }));
  }

  /** 起：登记进服务表，照依赖就绪或等着。 */
  start() {
    this.registry.fibers.add(this);
    this.refresh();
  }

  /** 这个服务变了要不要重新对一遍依赖（带 `~` 的不要）。 */
  needs(name) { return this.deps.some((d) => d.name === name && !d.lazy); }

  /** 缺哪些服务（带 `?` 的不算）。 */
  missing() {
    return this.deps.filter((d) => !d.optional && !this.registry.has(d.name)).map((d) => d.name);
  }

  /** 重新对一遍依赖：缺了撤回等着；都在、用着的变了，撤回重来。故障的、撤回了的不动。 */
  refresh() {
    if (this.state === 'disposed' || this.state === 'failed') return;
    const ready = this.missing().length === 0;
    const changed = this.deps.some((d) => !d.lazy && this.seen.get(d.name) !== this.registry.get(d.name));
    if (this.state === 'active' && ready && !changed) return;
    if (this.state === 'active') this.undo();
    if (!ready) {
      this.state = 'pending';
      return;
    }
    this.activate();
  }

  activate() {
    this.seen = new Map(this.deps.map((d) => [d.name, this.registry.get(d.name)]));
    this.state = 'active';
    this.reason = null;
    try {
      this.apply(this.context());
    } catch (err) {
      this.undo();
      this.state = 'failed';
      this.reason = err instanceof Error ? err.message : String(err);
    }
  }

  /** 撤回做过的：照反序；一件抛错记在控制台，别的照撤。 */
  undo() {
    const list = this.disposers.splice(0);
    for (const dispose of list.reverse()) {
      try {
        dispose();
      } catch (err) {
        console.error(`${this.manifest.id} 撤回时出错了`, err);
      }
    }
  }

  /** 只改了 `applies: live` 的设置项：换上新的值，告诉听的，不重来（加载器判断能不能这样换）。 */
  update(config) {
    this.config = Object.freeze({ ...config });
    for (const fn of [...this.configWatchers]) {
      try {
        fn(this.config);
      } catch (err) {
        console.error(`${this.manifest.id} 换设置项时出错了`, err);
      }
    }
  }

  /** 停用、卸载：撤回，离开服务表。 */
  dispose() {
    if (this.state === 'disposed') return;
    this.undo();
    this.state = 'disposed';
    this.registry.fibers.delete(this);
  }

  /** 给 `apply` 的上下文：自己的几个调用，加上 `inject` 写了的服务。 */
  context() {
    const fiber = this;
    const reg = this.registry;
    /** 按这个包绑的一份服务（服务有 `bind(ctx)` 的）：经它登记的东西跟着这个包撤回 */
    const bound = new Map();
    const own = {
      id: this.manifest.id,
      /** 设置项的最终值：总是现在的（改了 live 的项当场换） */
      get config() { return fiber.config; },
      /** 听设置项当场变（只改了 live 的项时）；跟着这个包撤回 */
      watchConfig: (fn) => own.effect(() => {
        fiber.configWatchers.add(fn);
        return () => fiber.configWatchers.delete(fn);
      }),
      /** 做一件事，`fn` 交回怎么撤回；交回的撤回只撤一次 */
      effect(fn) {
        const undo = fn();
        let done = false;
        const dispose = () => {
          if (done) return;
          done = true;
          const i = fiber.disposers.indexOf(dispose);
          if (i >= 0) fiber.disposers.splice(i, 1);
          if (typeof undo === 'function') undo();
        };
        fiber.disposers.push(dispose);
        return dispose;
      },
      provide: (name, value) => own.effect(() => {
        reg.set(name, value, fiber);
        return () => reg.unset(name, fiber);
      }),
      on: (event, fn) => own.effect(() => reg.on(event, fn)),
      emit: (event, ...args) => reg.emit(event, ...args),
      text: (path, fields) => lookup(fiber.manifest.text?.[LANGUAGE] ?? {}, path, fields),
    };
    const ctx = new Proxy(own, {
      get(target, key) {
        if (typeof key === 'symbol' || key === 'then') return undefined;
        if (key in target) return target[key];
        const dep = fiber.deps.find((d) => d.name === key);
        if (!dep) throw new Error(`${fiber.manifest.id} 没在清单的 inject 里写 ${key}`);
        const value = reg.get(key);
        // 带 `~` 的每次现找，不按包绑（它来来去去，绑的那份会过期）
        if (dep.lazy) return value;
        if (value && typeof value === 'object' && typeof value.bind === 'function') {
          if (!bound.has(key)) bound.set(key, value.bind(ctx));
          return bound.get(key);
        }
        return value;
      },
    });
    return ctx;
  }
}
