// @ts-check
//! 连核心：宿主给的一条线上的 JSON-RPC 2.0，一帧一条消息（`04-核心协议.md` 第二节、P2）。线是什么由宿主定（蓝图
//! `web/architecture.md`「宿主」）：浏览器是经桥的 WebSocket，桌面端是外壳的进程间通道；样子都照 WebSocket，这一层两边同一份。
//! 连上过又断了的（核心重启，桥把线关了），照 `retry` 隔一会儿自己再连，连上了告诉 `onReopen` 的（重新握手、补上漏掉的由外面做，
//! 蓝图 `web.md`「连核心」第 1 条）；一开始就连不上的不在这里重连（页面起不来，写一句）。重连没连上时问一句口令还对不对
//! （`check`），用不了了（桥重启过换了口令）不再白试，告诉 `onLost` 的（第 9 条）。

/** 核心拒绝的一个请求：`reason` 是稳定的原因码（蓝图 `protocol.md`「出错」），`message` 是核心按头的语言写的话。 */
export class Refusal extends Error {
  /** @param {string} message @param {number} code @param {string|null} reason @param {any} [data] 拒绝里别的几格（`data`，比如分块上传的 `received`） */
  constructor(message, code, reason, data = null) {
    super(message);
    this.code = code;
    this.reason = reason;
    this.data = data;
  }
}

/** 线通着（照 WebSocket 的 `OPEN`）。 */
const OPEN = 1;

export class Connection {
  /**
   * @param {() => import('../host/browser.js').Channel} open 开一条线（宿主给的）
   * @param {number[]} [retry] 断了以后第几次重连前等多久（毫秒），试完了照最后一个一直试；空的是不重连
   * @param {() => Promise<string>} [check] 问口令还对不对（宿主给的，桥的 `/key`）：`bad` 是用不了了
   */
  constructor(open, retry = [], check = async () => 'ok') {
    this.open = open;
    this.retry = retry;
    this.check = check;
    /** 口令用不了了：不再重连 */
    this.gaveUp = false;
    /** @type {Set<() => void>} */
    this.losts = new Set();
    /** 断了以后试了几次；连上过没有（连上过又断的才重连）；排着的下一次 */
    this.tries = 0;
    this.everOpen = false;
    this.retryTimer = /** @type {any} */ (null);
    /** @type {Set<() => void>} */
    this.reopens = new Set();
    this.n = 0;
    // 命令编号要自己保证不撞：每条连接取一段随机数当前缀（蓝图 `protocol.md`，照 `miyu ask`）
    this.prefix = Math.random().toString(16).slice(2, 10);
    /** @type {Map<string, {resolve: (v: any) => void, reject: (e: Error) => void}>} */
    this.waiting = new Map();
    /** @type {Set<(method: string, params: any) => void>} */
    this.pushes = new Set();
    /** @type {Set<(status: 'connecting'|'online'|'offline') => void>} */
    this.statuses = new Set();
    /** @type {import('../host/browser.js').Channel|null} */
    this.ws = null;
  }

  /**
   * 连上。
   *
   * # Errors
   * 线开不了（口令过期、桥没在跑）、那一头连不上核心（它推一条 `bridge.error`），照原因抛出来。
   */
  connect() {
    this.status('connecting');
    return new Promise((resolve, reject) => {
      const ws = this.open();
      this.ws = ws;
      let open = false;
      ws.onopen = () => {
        open = true;
        const again = this.everOpen;
        this.everOpen = true;
        this.tries = 0;
        this.status('online');
        resolve(undefined);
        if (again) for (const fn of this.reopens) fn();
      };
      ws.onerror = () => { if (!open) reject(new Error('bridge')); };
      ws.onclose = () => {
        if (!open) reject(new Error('bridge'));
        for (const w of this.waiting.values()) w.reject(new Refusal('核心断开了', -32000, 'disconnected'));
        this.waiting.clear();
        this.status('offline');
        this.later();
      };
      ws.onmessage = (ev) => this.receive(ev.data, reject);
    });
  }

  /** 收到一帧：回应交给等着它的请求；推送交给订了的；桥说连不上核心的，连接失败。 */
  receive(data, fail) {
    let m;
    try { m = JSON.parse(data); } catch { return; }
    if (m.method === 'bridge.error') {
      fail(new Error(m.params?.message ?? 'bridge'));
      return;
    }
    const waiter = m.id != null ? this.waiting.get(m.id) : undefined;
    if (waiter) {
      this.waiting.delete(m.id);
      if (m.error) waiter.reject(new Refusal(m.error.message, m.error.code, m.error.data?.reason ?? null, m.error.data ?? null));
      else waiter.resolve(m.result);
      return;
    }
    if (m.method) for (const fn of this.pushes) fn(m.method, m.params);
  }

  /**
   * 发一个请求，等它的回应。
   *
   * # Errors
   * 核心拒绝的抛 `Refusal`；连接断了抛原因码是 `disconnected` 的 `Refusal`。
   */
  request(method, params = {}) {
    const id = `web-${this.prefix}-${++this.n}`;
    return new Promise((resolve, reject) => {
      if (!this.ws || this.ws.readyState !== OPEN) {
        reject(new Refusal('核心断开了', -32000, 'disconnected'));
        return;
      }
      this.waiting.set(id, { resolve, reject });
      this.ws.send(JSON.stringify({ jsonrpc: '2.0', id, method, params }));
    });
  }

  /** 断了：连上过的，照 `retry` 隔一会儿再连（已经排着的不再排）。 */
  later() {
    if (!this.everOpen || !this.retry.length || this.retryTimer || this.gaveUp) return;
    const ms = this.retry[Math.min(this.tries, this.retry.length - 1)];
    this.tries += 1;
    this.retryTimer = setTimeout(() => {
      this.retryTimer = null;
      // 没连上：问一句口令还对不对，用不了了（桥重启过）不再白试，告诉外面
      this.connect().catch(async () => {
        if ((await this.check().catch(() => 'down')) !== 'bad') return;
        this.gaveUp = true;
        clearTimeout(this.retryTimer);
        this.retryTimer = null;
        for (const fn of this.losts) fn();
      });
    }, ms);
  }

  /** 口令用不了了（桥重启过），不再重连。 */
  onLost(fn) { this.losts.add(fn); }

  /** 断了又连上了（重新握手、重新订阅、补上漏掉的由它做）。 */
  onReopen(fn) { this.reopens.add(fn); }

  /** 收推送。 */
  onPush(fn) { this.pushes.add(fn); }

  /** 连接的状态变了：连接中、在线、离线（左栏的状态照它）。 */
  onStatus(fn) { this.statuses.add(fn); }

  status(s) { for (const fn of this.statuses) fn(s); }
}
