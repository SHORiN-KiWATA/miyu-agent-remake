// 接入QQ 的后台页（docs/blueprint/onebot.md 第一条「后台页」第 3 条，施工 O-28 上；docs/blueprint/package-pages.md「框和网页之间
// 怎么说」）：通道、字、颜色和明暗、页签、两页共用的状态。「连接」页在 connection.js，「白名单成员」页在 whitelist.js，换页时交给
// 它们要用的几样（`ui`）。都是原生的模块，index.html 照 type="module" 载这一个。
//
// - 页面跑在网页给的隔离框里（沙箱、来源是空的、不能联网）：只经网页交来的 MessageChannel 照 JSON-RPC 说。网页第一次载入完把
//   一头经 postMessage({miyu: 'port'}) 交过来，只交这一次：页面是单页，两个页签在页里切，不跳页（「施工时定的」第 155 条）。
// - 方法只有网页给的那几样：context、settings.get、settings.set、call（转 package.call，调桥登记的 status、connection.token）；
//   推送 settings.changed、theme.changed。
// - 字在 texts.js，照 context 的 language 挑；颜色照 context 的 colors，没给的照 theme 用 style.css 里的两套（第 159、160 条）。
import { TEXTS } from './texts.js';
import { connectionView } from './connection.js';
import { whitelistView } from './whitelist.js';

(() => {
  // NapCat 的状态隔多久调一次 status（同桥自己的网页：每 5 秒，「施工时定的」第 155 条）；「已复制」显示多久；context 的
  // colors 里每一种写进哪个 CSS 变量（style.css 里同名的那一套是没给颜色时的）。
  const POLL_MS = 5000;
  const COPIED_MS = 1500;
  const COLORS = {
    accent: '--accent', surface: '--surface', surface_2: '--surface-2', text: '--text', text_soft: '--text-soft',
    line: '--line', danger: '--danger',
  };

  const app = /** @type {HTMLElement} */ (document.getElementById('app'));

  /** 页面的全部状态：用哪一份字、通道、设置项的最终值（settings.get 的 values）、status 的回答和调不到时的错、取 status 的定时器、
   *  在哪一页、这一页（换页就换一份：画 `paint`、设置项变了 `changed`）、页签。 */
  const state = {
    said: pick(navigator.language), channel: null, values: {}, status: null, statusError: null, timer: 0,
    page: 'connection', view: null, tabs: {},
  };

  /** 照语言 `language`（`zh-CN` 这种取头一段）挑一份字；不认识的照英文。 */
  function pick(language) {
    const head = String(language ?? '').toLowerCase().split(/[-_]/)[0];
    return TEXTS[head] ?? TEXTS.en;
  }

  /** 编号 key 那一句，换进 fields（`{名字}`，`{{`、`}}` 是花括号）。没有这一句的照编号印。 */
  function say(key, fields = {}) {
    const template = state.said[key];
    if (template == null) return key;
    return template.replace(/\{\{|\}\}|\{([a-z_]+)\}/g, (match, name) => {
      if (match === '{{') return '{';
      if (match === '}}') return '}';
      return name in fields ? String(fields[name]) : match;
    });
  }

  /** 造一个元素：`text` 是字，`on…` 是事件，别的是属性；孩子是元素或字（字一律当字，不当 HTML）。 */
  function h(tag, props = {}, ...children) {
    const el = document.createElement(tag);
    for (const [key, value] of Object.entries(props)) {
      if (value == null || value === false) continue;
      if (key === 'class') el.className = value;
      else if (key === 'text') el.textContent = value;
      else if (key.startsWith('on')) el.addEventListener(key.slice(2), value);
      else el.setAttribute(key, value === true ? '' : String(value));
    }
    el.append(...children.flat().filter((child) => child != null && child !== false));
    return el;
  }

  /** 网页回的错的原话，问题一条条接在后面（核心拒绝 config.set 带的 data.problems，同桥自己的网页）。 */
  function reasonOf(error) {
    const problems = error?.data?.problems ?? [];
    return [error?.message ?? String(error), ...problems.map((p) => p.message).filter(Boolean)].join(' ');
  }

  /** 把 `text` 放进剪贴板：先用 navigator.clipboard；框是沙箱、多半没有它的许可，退到选中一段字 execCommand('copy')（用户刚按
   *  了按钮时还能用，「施工时定的」第 158 条）。都不成的抛错。 */
  async function copyText(text) {
    try {
      await navigator.clipboard.writeText(text);
      return;
    } catch { /* 框里不给：退到下面那一种 */ }
    const area = h('textarea', { class: 'offscreen', readonly: true, 'aria-hidden': 'true' });
    area.value = text;
    document.body.append(area);
    area.select();
    const copied = document.execCommand('copy');
    area.remove();
    if (!copied) throw new Error('copy');
  }

  /** 一个「复制」按钮：把 `text()` 交回的字（可以要等，例如先问桥）放进剪贴板，成了的字换成「已复制」一会儿；取不到、放不进的
   *  照旧（令牌能「显示」，人可以自己选）。 */
  function copyButton(text) {
    const button = h('button', { class: 'button', type: 'button', text: say('copy') });
    button.addEventListener('click', () => Promise.resolve().then(text).then(copyText).then(() => {
      button.textContent = say('copied');
      setTimeout(() => { button.textContent = say('copy'); }, COPIED_MS);
    }, () => { /* 取不到、放不进剪贴板：照旧 */ }));
    return button;
  }

  /** 问一句（页面里的对话框，跟着页面的颜色）：按 `yes` 那个交回真，按「取消」、Esc 交回假。先停在「取消」上。 */
  function confirmed(question, yes) {
    return new Promise((resolve) => {
      const button = (text, value, primary) => h('button', {
        class: primary ? 'button primary' : 'button', type: 'button', text, onclick: () => dialog.close(value),
      });
      const dialog = h('dialog', { class: 'confirm' },
        h('p', { text: question }),
        h('div', { class: 'actions' }, button(say('cancel'), 'no'), button(yes, 'yes', true)));
      dialog.addEventListener('close', () => {
        dialog.remove();
        resolve(dialog.returnValue === 'yes');
      });
      document.body.append(dialog);
      dialog.showModal();
    });
  }

  /** 网页交来的通道：JSON-RPC，框发请求、网页回；网页另发推送（不带 id），照方法名交给 `on` 登记的那一个。 */
  class Channel {
    constructor(port) {
      this.port = port;
      this.n = 0;
      this.waiting = new Map();
      this.handlers = {};
      port.onmessage = (event) => this.receive(event.data);
    }

    receive(message) {
      if (!message || typeof message !== 'object') return;
      if (message.id == null) {
        if (typeof message.method === 'string') this.handlers[message.method]?.(message.params ?? {});
        return;
      }
      const waiter = this.waiting.get(message.id);
      if (!waiter) return;
      this.waiting.delete(message.id);
      if (!message.error) return waiter.resolve(message.result);
      const { message: said, code, data } = message.error;
      waiter.reject(Object.assign(new Error(said ?? String(code)), { code, data: data ?? null }));
    }

    /** 发一个请求，等它的回应；网页回错的交回带 `code`、`data` 的错。 */
    call(method, params) {
      return new Promise((resolve, reject) => {
        const id = ++this.n;
        this.waiting.set(id, { resolve, reject });
        this.port.postMessage(params === undefined ? { jsonrpc: '2.0', id, method } : { jsonrpc: '2.0', id, method, params });
      });
    }

    /** 推送 `method` 来了交给 `handler`。 */
    on(method, handler) { this.handlers[method] = handler; }
  }

  /** 照网页的样子上色：`theme` 是 light、dark，`colors` 有的写进 CSS 变量，没有的去掉、照 style.css 那一套。 */
  function look(theme, colors) {
    const root = document.documentElement;
    root.dataset.theme = theme === 'dark' ? 'dark' : 'light';
    for (const [name, variable] of Object.entries(COLORS)) {
      const color = colors?.[name];
      if (typeof color === 'string' && color) root.style.setProperty(variable, color);
      else root.style.removeProperty(variable);
    }
  }

  /** 整页换成正中的一句话；`again` 有的多一个「重试」。 */
  function centered(text, again) {
    app.className = 'boot';
    app.replaceChildren(h('p', { class: again ? 'error' : 'boot-wait', text }),
      again && h('button', { class: 'button primary', type: 'button', text: say('retry'), onclick: again }));
  }

  /** 读一遍设置项的最终值，画上（这一页有 `changed` 的交给它）。 */
  async function reload() {
    const got = await state.channel.call('settings.get');
    state.values = got?.values ?? {};
  }

  /** 调一次桥登记的方法 `method`（经网页的 call 转 package.call），交回它的回答。 */
  const call = (method, params) => state.channel.call('call', params === undefined ? { method } : { method, params });

  /** 调一次 status，画上，交回它的回答（调不到的是 null，错记在 statusError）。 */
  async function refresh() {
    try {
      state.status = await call('status');
      state.statusError = null;
    } catch (error) {
      state.status = null;
      state.statusError = error;
    }
    state.view?.paint();
    return state.status;
  }

  /** 每 POLL_MS 调一次 status。 */
  async function poll() {
    clearTimeout(state.timer);
    await refresh();
    state.timer = setTimeout(poll, POLL_MS);
  }

  /** 两页要用的几样。 */
  const ui = {
    h, say, reasonOf, copyButton, confirmed, call, refresh,
    values: () => state.values,
    status: () => state.status,
    statusError: () => state.statusError,
    /** 写设置项（settings.set，照 config.set 写进系统配置；密钥给字的网页代存），成了重读一遍。 */
    save: async (changes) => {
      await state.channel.call('settings.set', { changes });
      await reload();
    },
  };

  /** 换到 `page` 那一页（connection、whitelist）：页签标上，页里换成它。 */
  function show(page) {
    state.page = page;
    for (const [name, tab] of Object.entries(state.tabs)) {
      tab.classList.toggle('is-on', name === page);
      tab.setAttribute('aria-selected', String(name === page));
    }
    state.view = page === 'whitelist' ? whitelistView(ui) : connectionView(ui);
    state.main.replaceChildren(...state.view.el);
    state.view.paint();
  }

  /** 画骨架：页顶一排页签，下面是页（框外是网页的顶栏、菜单，这里不另画一套，「施工时定的」第 163 条）。 */
  function shell() {
    const tab = (page, key) => h('button', { class: 'tab', type: 'button', role: 'tab', text: say(key), onclick: () => show(page) });
    state.tabs = { connection: tab('connection', 'nav/connection'), whitelist: tab('whitelist', 'nav/whitelist') };
    state.main = h('main', { class: 'page' });
    app.className = 'shell';
    app.replaceChildren(h('nav', { class: 'tabs', role: 'tablist' }, state.tabs.connection, state.tabs.whitelist), state.main);
    show(state.page);
  }

  /** 通道来了：问网页这时的样子，换字、上色，读设置项，画，开始调 status。推送照登记的办。 */
  async function start() {
    centered(say('loading'));
    try {
      const context = await state.channel.call('context');
      state.said = pick(context?.language);
      document.documentElement.lang = context?.language ?? '';
      look(context?.theme, context?.colors);
      await reload();
    } catch (error) {
      return centered(say('failed', { reason: reasonOf(error) }), start);
    }
    shell();
    poll();
  }

  /** 推送：主题换了跟着换；设置项变了重读，交给这一页（「施工时定的」第 161 条）。 */
  function listen(channel) {
    channel.on('theme.changed', (params) => look(params.theme, params.colors));
    channel.on('settings.changed', async () => {
      try { await reload(); } catch { return; /* 读不到：照手里的 */ }
      state.view?.changed?.();
    });
  }

  // 等网页交通道：只认父窗口交来的头一个（网页只交一次，之后再来的不算）。
  window.addEventListener('message', (event) => {
    if (state.channel || event.source !== window.parent || event.data?.miyu !== 'port' || !event.ports?.[0]) return;
    state.channel = new Channel(event.ports[0]);
    listen(state.channel);
    start();
  });
  centered(say('loading'));
})();
