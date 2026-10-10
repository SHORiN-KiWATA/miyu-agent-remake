// QQ 桥的 WebUI（docs/blueprint/onebot.md 第二条，施工 O-16）：登录、骨架、「连接」页；「终端管理员与白名单成员」页在 people.js（施工
// O-17），这里引进来，换页时交给它要用的几样。两个文件都是原生的模块（index.html 照 type="module" 载，「施工时定的」第 25 条）。
//
// - 页面自己说核心协议：经桥的 /ws 原样转给核心，握手带一次性码、用户名和密码，或者记住的登录令牌，核心验（照网页软件，
//   web-module.md 第一条）。读写配置、存密钥调核心现成的 config.get、config.set、secret.set，校验只在核心。
// - 只有桥知道的照桥自己给的，都带登录令牌：/status（NapCat 连没连上、令牌设没设，每 5 秒取一次）、/token（令牌的值：桥
//   自己的凭据，由桥交给登录了的管理员，不经核心协议）、/apply（端口存好以后问桥换完了没有）。配置改了，核心当场推给桥
//   （extension.config），桥照新的用，不用重启（施工 O-20）：/status、/token 答的就是桥手里最新的。
// - 页面里的字：登录以前照桥的 /human（这时还不能调 human.get），登录以后照 human.get；字放在
//   resources/software/onebot/human/，编号前缀 software/onebot/。
import { peoplePage } from './people.js';

(() => {
  // 握手报的协议主版本、头（protocol.md「握手」）；记住登录令牌的那一格（只在这个浏览器里，过期时刻由核心给，30 天）；
  // /status 隔多久取一次、断了隔多久重连（onebot.md 第二条「怎么走」第 3 条：每 5 秒）；字的编号前缀（store/resources.md
  // 「怎么走」第 3 条第 4 款）；「连接」页读写的三项配置；换令牌：多少个随机字节、存成哪个密钥（第 3 条）；「已复制」显示多久。
  const PROTOCOL = [1, 1];
  const HEAD = { kind: 'onebot-web', version: '0' };
  const STORE = 'miyu-onebot.login';
  const POLL_MS = 5000;
  const PREFIX = 'software/onebot/';
  const KEYS = { listen: 'onebot.listen', web: 'onebot.web', token: 'onebot.token' };
  const TOKEN_BYTES = 32;
  const SECRET = 'onebot';
  const COPIED_MS = 1500;

  const app = /** @type {HTMLElement} */ (document.getElementById('app'));

  /** 页面的全部状态：字（编号 → 模板）、核心连接、登录令牌（问桥时带它）、账号、config.get 的 items、/status 的回应（取不到
   *  的是 null）、显示着的令牌（收起的是空的）、没令牌时的三步在不在、取 /status 的定时器、骨架里要跟着状态改的几处、在哪一页
   *  （只记在页面里，重新载入回到「连接」，「施工时定的」第 34 条）、这一页里要跟着状态改的几处（换页就换一份）。 */
  const state = {
    said: {}, rpc: null, login: '', account: '', items: {}, status: null, shown: '', guide: false, timer: 0, parts: {},
    page: 'connection', view: {},
  };

  /** 编号 key（不带前缀）那一句，换进 fields。没有这一句的照编号印（照桥的 texts.rs）。 */
  function say(key, fields = {}) {
    const template = state.said[PREFIX + key];
    if (template == null) return key;
    return template.replace(/\{\{|\}\}|\{([a-z_]+)\}/g, (match, name) => {
      if (match === '{{') return '{';
      if (match === '}}') return '}';
      return name in fields ? String(fields[name]) : match;
    });
  }

  /** 换上一份字：`said` 是 /human、human.get 回的 `said`。 */
  function words(said, language) {
    state.said = said ?? {};
    if (language) document.documentElement.lang = language;
    document.title = say('web/brand');
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

  /** n 个随机字节写成十六进制。 */
  const hex = (n) => Array.from(crypto.getRandomValues(new Uint8Array(n)), (b) => b.toString(16).padStart(2, '0')).join('');

  /** 一个「复制」按钮：把 `text()` 交回的字（可以要等，例如先问桥）放进剪贴板（本机的 http 算安全的来源，有
   *  navigator.clipboard），成了的字换成「已复制」一会儿；取不到、放不进的照旧。 */
  function copyButton(text) {
    const button = h('button', { class: 'button', type: 'button', text: say('web/copy') });
    button.addEventListener('click', () => Promise.resolve().then(text)
      .then((value) => navigator.clipboard.writeText(value))
      .then(() => {
        button.textContent = say('web/copied');
        setTimeout(() => { button.textContent = say('web/copy'); }, COPIED_MS);
      }, () => { /* 取不到、放不进剪贴板：人可以按「显示」自己选中复制 */ }));
    return button;
  }

  /** 问一句（页面里的对话框，跟着页面的亮暗色）：按 `yes` 那个交回真，按「取消」、Esc 交回假。先停在「取消」上。 */
  function confirmed(question, yes) {
    return new Promise((resolve) => {
      const button = (text, value, primary) => h('button', {
        class: primary ? 'button primary' : 'button', type: 'button', text, onclick: () => dialog.close(value),
      });
      const dialog = h('dialog', { class: 'confirm' },
        h('p', { text: question }),
        h('div', { class: 'actions' }, button(say('web/cancel'), 'no'), button(yes, 'yes', true)));
      dialog.addEventListener('close', () => {
        dialog.remove();
        resolve(dialog.returnValue === 'yes');
      });
      document.body.append(dialog);
      dialog.showModal();
    });
  }

  /** 核心拒绝的话：原话，问题一条条接在后面（config.set 写错了的，onebot.md 第二条「怎么走」第 3 条）。 */
  function reasonOf(error) {
    const problems = error?.data?.problems ?? [];
    return [error?.message ?? String(error), ...problems.map((p) => p.message).filter(Boolean)].join(' ');
  }

  /** 记住的、没过期的登录令牌；没有的、读不了的是空的。 */
  function remembered() {
    try {
      const saved = JSON.parse(localStorage.getItem(STORE) ?? 'null');
      if (saved?.token && Date.parse(saved.expires) > Date.now()) return saved.token;
    } catch { /* 隐私窗口、清过数据：当没有 */ }
    forget();
    return '';
  }

  /** 记住核心给的登录令牌 `{token, expires}`。记不住的（隐私窗口）下次再登。 */
  function remember(login) {
    try { localStorage.setItem(STORE, JSON.stringify(login)); } catch { /* 记不住：下次再登 */ }
  }
  function forget() {
    try { localStorage.removeItem(STORE); } catch { /* 本来就没有 */ }
  }

  /** 一条经 /ws 的核心连接：JSON-RPC，一帧一条；回应照编号交回。`failure` 是桥说的连不上核心（web.error），`onclose`
   *  是断了告诉谁。 */
  class Rpc {
    constructor() {
      this.ws = new WebSocket(`ws://${location.host}/ws`);
      this.n = 0;
      this.prefix = hex(4);
      this.waiting = new Map();
      this.failure = null;
      this.onclose = null;
      this.ws.addEventListener('message', (event) => this.receive(event.data));
      this.ws.addEventListener('close', () => {
        const why = this.failure ?? new Error(say('web/core-lost'));
        for (const waiter of this.waiting.values()) waiter.reject(why);
        this.waiting.clear();
        this.onclose?.();
      });
    }

    receive(data) {
      let message;
      try { message = JSON.parse(data); } catch { return; }
      if (message.method === 'web.error') return void (this.failure = new Error(message.params?.message ?? ''));
      const waiter = message.id != null ? this.waiting.get(message.id) : undefined;
      if (!waiter) return;
      this.waiting.delete(message.id);
      if (!message.error) return waiter.resolve(message.result);
      const detail = message.error.data ?? null;
      waiter.reject(Object.assign(new Error(message.error.message), { reason: detail?.reason ?? null, data: detail }));
    }

    /** 发一个请求，等它的回应；拒绝的交回带 `reason` 的错。 */
    call(method, params = {}) {
      return new Promise((resolve, reject) => {
        if (this.ws.readyState > WebSocket.OPEN) return reject(this.failure ?? new Error(say('web/core-lost')));
        const id = `onebot-web-${this.prefix}-${++this.n}`;
        this.waiting.set(id, { resolve, reject });
        const send = () => this.ws.send(JSON.stringify({ jsonrpc: '2.0', id, method, params }));
        if (this.ws.readyState === WebSocket.CONNECTING) this.ws.addEventListener('open', send, { once: true });
        else send();
      });
    }

    /** 自己关的：不当成断了。 */
    close() { this.onclose = null; this.ws.close(); }
  }

  /** 连上、握手（凭据照 `credentials`）：交回连接和握手的回应。被拒的抛带 `reason` 的错；连不上的（桥不在、桥连不上
   *  核心）连接一关，在等的握手跟着抛错。 */
  async function connect(credentials) {
    const rpc = new Rpc();
    const hello = await rpc.call('hello', { protocol: PROTOCOL, head: HEAD, locale: navigator.language, ...credentials });
    return { rpc, hello };
  }

  /** 起来：先要字，再照地址栏的一次性码、记住的登录令牌进，都没有的问用户名和密码。 */
  async function boot() {
    try {
      const answer = await fetch('/human', { cache: 'no-store' });
      if (!answer.ok) throw new Error(`/human ${answer.status}`);
      const got = await answer.json();
      words(got.said, got.language);
    } catch (error) {
      return centered(h('p', { class: 'boot-wait', text: String(error) }));
    }
    // 一次性码在 # 后面，不发给服务器；拿到就从地址栏抹掉（web-ui.md「怎么走」第二条第 3 款）。
    const code = new URLSearchParams(location.hash.slice(1)).get('setup');
    if (code) {
      history.replaceState(null, '', location.pathname + location.search);
      return start({ code });
    }
    const login = remembered();
    if (login) return start({ login });
    showLogin('');
  }

  /** 照 `credentials` 进门。 */
  async function start(credentials) {
    waiting();
    let session;
    try {
      session = await connect(credentials);
    } catch (error) {
      if (credentials.login && error.reason === 'bad_login') forget();
      if (error.reason || credentials.code) return showLogin(error.message);
      return showRetry(error.message, () => start(credentials));
    }
    const { rpc, hello } = session;
    if (hello.setup) return showSetup(rpc, hello.account);
    if (hello.login?.token) remember(hello.login);
    enter(rpc, hello.account, hello.login?.token ?? credentials.login);
  }

  /** 整页换成正中的一句话（`boot-wait`）或者一张卡片。 */
  function centered(el) {
    app.className = 'boot';
    app.replaceChildren(el);
  }
  const waiting = () => centered(h('p', { class: 'boot-wait', text: say('web/loading') }));
  const card = (...children) => centered(h('div', { class: 'boot-card' }, ...children));

  /** 连不上（桥不在、桥连不上核心）：说原话，给一个「重试」。 */
  function showRetry(message, again) {
    const retry = h('button', { class: 'button primary', type: 'button', text: say('web/retry'), onclick: again });
    card(h('p', { class: 'error', text: message }), h('div', { class: 'actions' }, retry));
  }

  /** 一张表单：标题、几格、按钮，提交时交给 `submit`（拿到几格的值）。 */
  function form(title, hint, fields, label, submit, message) {
    const inputs = fields.map((field) => h('input', field.attrs));
    const error = h('p', { class: 'error', text: message });
    const button = h('button', { class: 'button primary', type: 'submit', text: label });
    const el = h('form', { class: 'login' },
      h('h1', { text: title }),
      hint && h('p', { class: 'hint', text: hint }),
      fields.map((field, at) => h('label', {}, h('span', { text: field.label }), inputs[at])),
      error,
      h('div', { class: 'actions' }, button),
    );
    el.addEventListener('submit', async (event) => {
      event.preventDefault();
      button.disabled = true;
      error.textContent = '';
      try {
        await submit(inputs.map((input) => input.value));
      } catch (failed) {
        error.textContent = failed.message;
      } finally {
        button.disabled = false;
      }
    });
    card(el);
    inputs[0]?.focus();
  }

  /** 用户名、密码两格；`password` 是密码那一格的 autocomplete，`user` 是用户名的初值。 */
  function credentials(password, user) {
    return [
      { label: say('web/login/user'), attrs: { name: 'user', autocomplete: 'username', required: true, value: user } },
      { label: say('web/login/password'), attrs: { name: 'password', type: 'password', autocomplete: password, required: true } },
    ];
  }

  /** 问用户名和密码；还没设过密码的提示去终端里要一次性码（onebot.md 第二条「怎么走」第 2 条）。 */
  function showLogin(message) {
    const fields = credentials('current-password');
    form(say('web/login/title'), say('web/login/no-password'), fields, say('web/login/submit'), async ([user, password]) => {
      const { rpc, hello } = await connect({ user, password });
      if (hello.login?.token) remember(hello.login);
      enter(rpc, hello.account, hello.login?.token ?? '');
    }, message);
  }

  /** 用一次性码连上的：先设用户名和密码（account.setup），换到登录令牌。用户名的初值是账号的编号（web-module.md 第一条：
   *  默认 admin）。 */
  function showSetup(rpc, account) {
    form(say('web/setup/title'), say('web/setup/hint'), credentials('new-password', account), say('web/setup/submit'),
      async ([username, password]) => {
        const done = await rpc.call('account.setup', { username, password });
        remember(done.login);
        enter(rpc, account, done.login.token);
      }, '');
  }

  /** 进来了：照连接的语言换字，读配置，画骨架和在看的那一页，开始取 /status。 */
  async function enter(rpc, account, login) {
    state.rpc = rpc;
    state.account = account ?? '';
    state.login = login;
    rpc.onclose = lost;
    try {
      const human = await rpc.call('human.get', {});
      words(human.said, human.language);
    } catch { /* 换不了就照 /human 的字 */ }
    try { await load(); } catch (error) { return showRetry(reasonOf(error), () => start({ login })); }
    shell();
    poll();
  }

  /** 读配置，画上（在「连接」页的话）。不写 keys：要全部，终端管理员表有哪些号事先不知道，两页共用这一份（「施工时定的」第 33 条）。 */
  async function load() {
    const got = await state.rpc.call('config.get', {});
    state.items = got.items ?? {};
    state.view.paintConfig?.();
  }

  /** 带着登录令牌问桥（/token、/apply）。登录令牌不认了（401）的回登录，抛错。 */
  async function bridge(path, method = 'GET') {
    const answer = await fetch(path, { method, headers: { Authorization: `Bearer ${state.login}` }, cache: 'no-store' });
    if (answer.status === 401) {
      signedOut();
      throw new Error(say('web/login/title'));
    }
    return answer;
  }

  /** 桥手里的令牌（/token：核心换上新配置就推给桥，刚写进去的令牌当场照新的）。没有的交回空的。 */
  async function tokenValue() {
    const answer = await bridge('/token');
    if (!answer.ok) throw new Error(`/token ${answer.status}`);
    return (await answer.json()).token ?? '';
  }

  /** 问桥两个端口换完了没有（/apply：照桥手里最新的配置换，核心推来的多半已经换好）。交回实际听的 {listen, web}；新端口被
   *  占的抛「被占了」的话。WebUI 的端口已经被推来的换掉、旧地址上这一问回 403（Host 对不上新端口）或连不上的，交回空的：
   *  调的一方照刚存的端口跳（onebot.md 第二条「施工时定的」第 37 条）。 */
  async function apply() {
    const answer = await bridge('/apply', 'POST').catch((error) => {
      if (error instanceof TypeError) return null;
      throw error;
    });
    if (!answer || answer.status === 403) return null;
    if (answer.status === 409) throw new Error(say('web/ports/in-use', { port: (await answer.json()).in_use }));
    if (!answer.ok) throw new Error(`/apply ${answer.status}`);
    return answer.json();
  }

  /** 核心那头断了（核心重启、关 1012）：隔一会儿用登录令牌重连。 */
  function lost() {
    clearTimeout(state.timer);
    state.parts.banner?.replaceChildren(say('web/core-lost'));
    state.parts.banner?.removeAttribute('hidden');
    setTimeout(async () => {
      try {
        const { rpc, hello } = await connect({ login: state.login });
        enter(rpc, hello.account, state.login);
      } catch (error) {
        if (error.reason) { forget(); return showLogin(error.message); }
        lost();
      }
    }, POLL_MS);
  }

  /** 取一次 /status，画上，隔 POLL_MS 再取。登录令牌不认了的回登录；取的时候已经退出了的、换了令牌的，不再画。 */
  async function poll() {
    clearTimeout(state.timer);
    const login = state.login;
    if (!login) return;
    let status = null;
    try {
      const answer = await fetch('/status', { headers: { Authorization: `Bearer ${login}` }, cache: 'no-store' });
      if (state.login !== login) return;
      if (answer.status === 401) return signedOut();
      status = answer.ok ? await answer.json() : null;
    } catch { /* 桥不在：照不知道画 */ }
    if (state.login !== login) return;
    state.status = status;
    paintStatus();
    state.timer = setTimeout(poll, POLL_MS);
  }

  /** 一个圆点：连着、没连着、不知道。 */
  function dot(kind) {
    return h('span', { class: `status-dot is-${kind}` });
  }

  /** 一行：左边的名字，右边的几样。 */
  function row(label, ...values) {
    return h('div', { class: 'row' }, h('span', { class: 'label', text: label }), ...values);
  }

  /** 画骨架：顶栏、左栏、页；页里照 state.page 画（`show`）。 */
  function shell() {
    const parts = state.parts = {};
    parts.pill = h('span', { class: 'pill' });
    parts.banner = h('div', { class: 'banner', hidden: true });
    parts.main = h('main', { class: 'page' });
    const menu = h('div', { class: 'menu', hidden: true }, h('button', { type: 'button', text: say('web/sign-out'), onclick: signOut }));
    const drawer = () => app.classList.toggle('drawer');
    const toggle = h('button', { class: 'icon-button', type: 'button', 'aria-label': say('web/menu'), onclick: drawer }, h('span', { class: 'burger' }));
    const off = (key) => h('span', { class: 'nav-item is-off', title: say('web/not-yet'), 'aria-disabled': 'true', text: say(key) });
    const go = (page, key) => h('button', { class: 'nav-item', type: 'button', text: say(key), onclick: () => show(page) });
    parts.nav = { connection: go('connection', 'web/nav/connection'), people: go('people', 'web/nav/people') };
    app.className = 'shell';
    app.replaceChildren(
      h('header', { class: 'top' },
        toggle,
        h('span', { class: 'brand', text: say('web/brand') }),
        parts.pill,
        h('span', { class: 'grow' }),
        h('div', { class: 'account' },
          h('button', { class: 'account-button', type: 'button', text: `${state.account} ▾`, onclick: () => menu.toggleAttribute('hidden') }),
          menu)),
      h('div', { class: 'frame' },
        h('nav', { class: 'side' },
          parts.nav.connection,
          parts.nav.people,
          h('span', { class: 'nav-sep', text: say('web/later') }),
          ['venues', 'tools', 'plugins', 'logs'].map((page) => off(`web/nav/${page}`))),
        h('div', { class: 'scrim', onclick: () => app.classList.remove('drawer') }),
        parts.main)
    );
    show(state.page);
  }

  /** 换到 `page` 那一页（connection、people）：左栏标上，页里换成它，窄屏收起抽屉。这一页要跟着状态改的几处放进新的一份
   *  state.view，上一页的随它丢掉。 */
  function show(page) {
    state.page = page;
    state.view = {};
    app.classList.remove('drawer');
    for (const [name, item] of Object.entries(state.parts.nav)) {
      item.classList.toggle('is-on', name === page);
      if (name === page) item.setAttribute('aria-current', 'page');
      else item.removeAttribute('aria-current');
    }
    const body = page === 'people' ? peopleView(state.view) : connectionView(state.view);
    state.parts.main.replaceChildren(state.parts.banner, ...body);
    paintStatus();
  }

  /** 「连接」页：没令牌时的三步、NapCat、要填的、端口。 */
  function connectionView(view) {
    view.napcat = h('div', { class: 'value' });
    view.step = h('li', { text: say('web/guide/token') });
    view.guide = h('section', { class: 'card guide', hidden: true },
      h('h2', { text: say('web/guide/title') }),
      h('ol', {}, view.step, h('li', { text: say('web/guide/napcat') }), h('li', { text: say('web/guide/wait') })));
    return [
      h('h1', { text: say('web/nav/connection') }),
      view.guide,
      h('section', { class: 'card' }, row(say('web/napcat'), view.napcat)),
      fillCard(view),
      portsCard(),
    ];
  }

  /** 「终端管理员与白名单成员」页（people.js）：先读一遍配置；平台的名字照 /status 的 platform（「施工时定的」第 26 条），两样都到了
   *  才画，之前说「正在连」。存表经核心的 config.set，只写系统配置，存好了重读配置。 */
  function peopleView(view) {
    const body = h('div', {}, h('p', { class: 'boot-wait', text: say('web/loading') }));
    let loaded = false;
    view.status = () => {
      if (state.view !== view || !loaded || !state.status?.platform || view.drawn) return;
      view.drawn = true;
      body.replaceChildren(...peoplePage({
        h,
        say,
        platform: state.status.platform,
        account: state.account,
        items: () => state.items,
        save: async (changes) => {
          await state.rpc.call('config.set', { layer: 'system', changes });
          await load();
        },
        failed: (error) => say('web/failed', { reason: reasonOf(error) }),
      }));
    };
    load().then(() => {
      loaded = true;
      view.status();
    }, (error) => {
      if (state.view === view) body.replaceChildren(h('p', { class: 'error', text: say('web/failed', { reason: reasonOf(error) }) }));
    });
    return [h('h1', { text: say('web/nav/people') }), body];
  }

  /** NapCat 那边要填的：地址（照 onebot.listen 拼）、令牌、消息格式。配置读回来、/status 取回来以后照 `view.paintConfig`
   *  重画地址和令牌。令牌照 /status 的 token（onebot.md 第二条「怎么走」第 3 条）：设了的「显示」「复制」「换一个」（换之前
   *  问一句），没设、取不到的只有一个主按钮「生成」、不问：没有旧的要换掉。 */
  function fillCard(view) {
    const url = () => say('web/fill/url', { port: state.items[KEYS.listen]?.value ?? '' });
    const address = h('code');
    const token = h('div', { class: 'value' });
    const note = h('p', { class: 'error' });
    const fail = (error) => { note.textContent = say('web/failed', { reason: reasonOf(error) }); };
    const show = h('button', { class: 'button', type: 'button' });
    const copy = copyButton(tokenValue);
    const change = h('button', { class: 'button', type: 'button', text: say('web/token/change') });
    const create = h('button', { class: 'button primary', type: 'button', text: say('web/token/generate') });
    // 每次取回 /status 都重画：字没变的不动，免得人正选着的地址、令牌被换掉。
    let painted = '';
    const paint = view.paintConfig = () => {
      const { text, set } = tokenState();
      const shown = set && state.shown;
      if (address.textContent !== url()) address.textContent = url();
      if (painted !== `${shown}\n${text}`) {
        painted = `${shown}\n${text}`;
        token.replaceChildren(shown ? h('code', { text: shown }) : h('span', { text }));
      }
      show.textContent = say(state.shown ? 'web/token/hide' : 'web/token/show');
      for (const button of [show, copy, change]) button.hidden = !set;
      create.hidden = set || !state.status;
    };
    show.addEventListener('click', async () => {
      note.textContent = '';
      try { state.shown = state.shown ? '' : await tokenValue(); } catch (error) { fail(error); }
      paint();
    });
    // 生成：写进核心（secret.set、config.set），再问桥要回来显示：核心换上新配置就推给桥，NapCat 下一次连就照新的。
    const generate = async (asking) => {
      if (asking && !(await confirmed(say('web/token/confirm'), say('web/token/change')))) return;
      change.disabled = create.disabled = true;
      note.textContent = '';
      try {
        const fresh = hex(TOKEN_BYTES);
        await state.rpc.call('secret.set', { name: SECRET, value: fresh });
        await state.rpc.call('config.set', { layer: 'system', changes: [{ key: KEYS.token, value: { secret: SECRET } }] });
        state.shown = await tokenValue();
        await load();
        await poll();
      } catch (error) {
        fail(error);
      } finally {
        change.disabled = create.disabled = false;
        paint();
      }
    };
    change.addEventListener('click', () => generate(true));
    create.addEventListener('click', () => generate(false));
    paint();
    return h('section', { class: 'card' },
      h('h2', { text: say('web/fill/title') }),
      row(say('web/fill/address'), h('div', { class: 'value line' }, address, copyButton(url))),
      row(say('web/fill/token'), token, h('div', { class: 'line' }, show, copy, change, create)),
      note,
      row(say('web/fill/format'), h('span', { class: 'value', text: say('web/fill/array') })));
  }

  /** 令牌那一行：照 /status 的 token（设了、没有）和配置里的引用（照环境变量的写名字）。交回那一行的字、设没设；还没取到
   *  /status 的字是空的。没写引用、引用取不到，核心都不交给桥，/status 都说 none（施工 O-20）。 */
  function tokenState() {
    const reference = state.items[KEYS.token]?.value;
    const kind = state.status?.token;
    if (kind === 'set') {
      const text = reference?.env ? say('web/token/env', { name: reference.env }) : `${say('web/token/set')} ········`;
      return { text, set: true };
    }
    return { text: kind === 'none' ? say('web/token/unset') : '', set: false };
  }

  /** 两个端口：写错的核心回问题，照原话说。 */
  function portsCard() {
    const input = (key) => h('input', { type: 'number', min: 1024, max: 65535, inputmode: 'numeric', value: state.items[key]?.value ?? '' });
    const listen = input(KEYS.listen);
    const web = input(KEYS.web);
    const note = h('p', { class: 'note' });
    const save = h('button', { class: 'button primary', type: 'submit', text: say('web/save') });
    const el = h('form', { class: 'card' },
      h('h2', { text: say('web/ports/title') }),
      h('div', { class: 'ports' },
        h('label', {}, h('span', { text: say('web/ports/listen') }), listen),
        h('label', {}, h('span', { text: say('web/ports/web') }), web),
        save),
      note);
    // 存好了核心当场推给桥、桥照新的换，再问桥换完了没有（/apply）：WebUI 的端口换了，先说一句再跳过去（登录记在浏览器里、
    // 按地址分，新地址上要再登录）；桥已经换到新端口、旧地址上问不到的，照刚存的端口跳；NapCat 的换了，提醒去 NapCat 里改
    // 地址。被占的照 /apply 的 409 说，桥照旧用原来的。
    el.addEventListener('submit', async (event) => {
      event.preventDefault();
      const changes = [[KEYS.listen, listen], [KEYS.web, web]]
        .filter(([key, field]) => String(state.items[key]?.value ?? '') !== field.value)
        .map(([key, field]) => ({ key, input: field.value }));
      if (!changes.length) return;
      save.disabled = true;
      note.className = 'note';
      try {
        await state.rpc.call('config.set', { layer: 'system', changes });
        const moved = await apply();
        const saved = changes.find((change) => change.key === KEYS.web);
        const webPort = moved?.web ?? (saved && Number(saved.input));
        if (!webPort) throw new Error('/apply');
        await load();
        if (webPort !== Number(location.port)) {
          alert(say('web/ports/web-moved', { port: webPort }));
          location.assign(`${location.protocol}//${location.hostname}:${webPort}/`);
          return;
        }
        const listenMoved = changes.some((change) => change.key === KEYS.listen);
        note.textContent = listenMoved ? say('web/ports/listen-moved', { port: moved.listen }) : say('web/saved');
        poll();
      } catch (error) {
        note.className = 'note error';
        note.textContent = say('web/failed', { reason: reasonOf(error) });
      } finally {
        save.disabled = false;
      }
    });
    return el;
  }

  /** 照 /status 画顶栏的那一格；在「连接」页的再画 NapCat 那一行、令牌那一行、没令牌时的三步：令牌不是 set 的时候出来，
   *  出来了就留到 NapCat 连上（令牌设好了第一步打勾），这一页里不再出来（onebot.md 第二条「施工时定的」第 22 条）；在
   *  「终端管理员与白名单成员」页的，等着平台名字的那一页照它画。 */
  function paintStatus() {
    const { pill } = state.parts;
    if (!pill) return;
    const status = state.status;
    const napcatStatus = status?.napcat;
    const kind = !status ? 'unknown' : napcatStatus?.connected ? 'online' : 'offline';
    const said = { unknown: say('web/napcat/unknown'), online: say('web/napcat/connected'), offline: say('web/napcat/disconnected') }[kind];
    pill.replaceChildren(dot(kind), `${say('web/napcat')} ${said}`);
    // 三步出不出来照状态记着，不在「连接」页也照记：换回来时照实画。
    if (status?.token && status.token !== 'set') state.guide = true;
    if (napcatStatus?.connected) state.guide = false;
    const { napcat, guide, step } = state.view;
    if (napcat) {
      const detail = !napcatStatus?.connected ? ''
        : napcatStatus.version ? say('web/napcat/detail', { implementation: napcatStatus.implementation, version: napcatStatus.version, bot: napcatStatus.self_id })
          : say('web/napcat/bot', { bot: napcatStatus.self_id });
      napcat.replaceChildren(dot(kind), h('span', { text: said }), detail && h('span', { class: 'detail', text: detail }));
      guide.hidden = !state.guide;
      step.classList.toggle('done', status?.token === 'set');
    }
    state.view.paintConfig?.();
    state.view.status?.();
  }

  /** 退出这一个浏览器的登录（account.logout 不写 all：只作废这一个），回登录。 */
  async function signOut() {
    clearTimeout(state.timer);
    // 核心回完就断开这条连接：不当成核心断了去重连。
    if (state.rpc) state.rpc.onclose = null;
    try { await state.rpc?.call('account.logout', {}); } catch { /* 核心回完就断开：照样回登录 */ }
    signedOut();
  }

  /** 登录不算数了（退出了、桥说令牌不认）：忘掉登录令牌和显示着的 NapCat 令牌、关连接，回登录。 */
  function signedOut() {
    clearTimeout(state.timer);
    forget();
    state.rpc?.close();
    state.rpc = null;
    state.login = '';
    state.shown = '';
    showLogin('');
  }

  boot();
})();
