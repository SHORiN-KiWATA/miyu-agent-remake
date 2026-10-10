// 接入QQ 后台页的「连接」页（docs/blueprint/onebot.md 第一条「后台页」第 3 条，施工 O-28 上）：没令牌时的三步、NapCat 的状态、
// NapCat 那边要填的（地址、令牌、消息格式）、NapCat 的端口。照原来桥自己的网页的「连接」页（O-28 下去掉了），换成经网页的
// 通道说：状态、令牌的值调桥登记的 status、connection.token，令牌、端口经 settings.set 写。
//
// 骨架、字、通道都在 app.js，经 connectionView 的参数交过来；这里不碰全局的东西。

/** 「连接」页读写的两项设置。 */
const KEYS = { listen: 'onebot.listen', token: 'onebot.token' };

/** 生成令牌：多少个随机字节，写成十六进制（同桥自己的网页，施工单「要定的」第 1 条）。 */
const TOKEN_BYTES = 32;

/** 端口存好以后隔多久调一次 status、最多几次，看桥换没换成（「施工时定的」第 155、156 条）。 */
const MOVE_CHECK_MS = 500;
const MOVE_CHECKS = 10;

/** n 个随机字节写成十六进制。 */
const hex = (n) => Array.from(crypto.getRandomValues(new Uint8Array(n)), (b) => b.toString(16).padStart(2, '0')).join('');

/** 等 ms 毫秒。 */
const sleep = (ms) => new Promise((resolve) => { setTimeout(resolve, ms); });

/** 没令牌时的三步要不要出来：令牌不是 set 的时候出来，出来了就留到 NapCat 连上（「后台页」第 3 条「连接」）。在这个框里
 *  记着：换页再回来照实画。 */
let guided = false;

/** 「连接」页。`ui`：`h`、`say`、`reasonOf`、`copyButton`、`confirmed`（app.js 的），`call(method)`（调桥登记的方法），
 *  `refresh()`（调一次 status、画上），`values()`、`status()`、`statusError()`（这时的），`save(changes)`（settings.set、重读）。
 *  交回 `{el, paint, changed}`。 */
export function connectionView(ui) {
  const { h, say } = ui;
  const row = (label, ...values) => h('div', { class: 'row' }, h('span', { class: 'label', text: label }), ...values);
  const dot = (kind) => h('span', { class: `status-dot is-${kind}` });

  const napcat = h('div', { class: 'value' });
  const step = h('li', { text: say('guide/token') });
  const guide = h('section', { class: 'card guide', hidden: true },
    h('h2', { text: say('guide/title') }),
    h('ol', {}, step, h('li', { text: say('guide/napcat') }), h('li', { text: say('guide/wait') })));

  // NapCat 那边要填的：地址照 status 的 listen、path 拼；令牌照 status 的 token（第一条「后台页」第 3 条）。
  let shown = '';
  const address = h('code');
  const url = () => {
    const status = ui.status();
    return status ? say('fill/url', { port: status.listen, path: status.path }) : '';
  };
  const token = h('div', { class: 'value' });
  const note = h('p', { class: 'error' });
  const fail = (error) => { note.textContent = say('failed', { reason: ui.reasonOf(error) }); };
  const tokenValue = async () => (await ui.call('connection.token'))?.token ?? '';
  const show = h('button', { class: 'button', type: 'button' });
  const copy = ui.copyButton(tokenValue);
  const change = h('button', { class: 'button', type: 'button', text: say('token/change') });
  const create = h('button', { class: 'button primary', type: 'button', text: say('token/generate') });

  /** 令牌那一行的字、设没设：照 status 的 token 和设置项里的引用（照环境变量的写名字）。还没调到 status 的字是空的。 */
  function tokenState() {
    const kind = ui.status()?.token;
    if (kind === 'set') {
      const reference = ui.values()[KEYS.token];
      const text = reference?.env ? say('token/env', { name: reference.env }) : `${say('token/set')} ········`;
      return { text, set: true };
    }
    return { text: kind === 'none' ? say('token/unset') : '', set: false };
  }

  // 每次调回 status 都重画：字没变的不动，免得人正选着的地址、令牌被换掉。
  let painted = '';
  function paint() {
    const status = ui.status();
    const napcatStatus = status?.napcat;
    const kind = !status ? 'unknown' : napcatStatus?.connected ? 'online' : 'offline';
    const detail = !napcatStatus?.connected ? ''
      : napcatStatus.version ? say('napcat/detail', { implementation: napcatStatus.implementation, version: napcatStatus.version, bot: napcatStatus.self_id })
        : say('napcat/bot', { bot: napcatStatus.self_id });
    const said = kind === 'online' ? say('napcat/connected') : kind === 'offline' ? say('napcat/disconnected')
      : ui.statusError() ? say('napcat/unknown', { reason: ui.reasonOf(ui.statusError()) }) : '';
    napcat.replaceChildren(dot(kind), h('span', { text: said }), detail && h('span', { class: 'detail', text: detail }));
    if (status?.token && status.token !== 'set') guided = true;
    if (napcatStatus?.connected) guided = false;
    guide.hidden = !guided;
    step.classList.toggle('done', status?.token === 'set');
    if (address.textContent !== url()) address.textContent = url();
    const { text, set } = tokenState();
    const visible = set ? shown : '';
    if (painted !== `${visible}\n${text}`) {
      painted = `${visible}\n${text}`;
      token.replaceChildren(visible ? h('code', { text: visible }) : h('span', { text }));
    }
    show.textContent = say(shown ? 'token/hide' : 'token/show');
    for (const button of [show, copy, change]) button.hidden = !set;
    create.hidden = set || !status;
  }

  show.addEventListener('click', async () => {
    note.textContent = '';
    try { shown = shown ? '' : await tokenValue(); } catch (error) { fail(error); }
    paint();
  });
  // 生成、换：页面生成，settings.set 交给网页代存成密钥、写成引用；成了显示生成的这一个（桥收推送和答方法走两条路，刚写完就调
  // connection.token 可能拿到旧的，「施工时定的」第 157 条），再调 status。
  const generate = async (asking) => {
    if (asking && !(await ui.confirmed(say('token/confirm'), say('token/change')))) return;
    change.disabled = create.disabled = true;
    note.textContent = '';
    try {
      const fresh = hex(TOKEN_BYTES);
      await ui.save([{ key: KEYS.token, value: fresh }]);
      shown = fresh;
      await ui.refresh();
    } catch (error) {
      fail(error);
    } finally {
      change.disabled = create.disabled = false;
      paint();
    }
  };
  change.addEventListener('click', () => generate(true));
  create.addEventListener('click', () => generate(false));

  const fill = h('section', { class: 'card' },
    h('h2', { text: say('fill/title') }),
    row(say('fill/address'), h('div', { class: 'value line' }, address, ui.copyButton(url))),
    row(say('fill/token'), token, h('div', { class: 'line' }, show, copy, change, create)),
    note,
    row(say('fill/format'), h('span', { class: 'value', text: say('fill/array') })));

  const ports = portsCard(ui);
  return {
    el: [guide, h('section', { class: 'card' }, row(say('napcat'), napcat)), fill, ports.el],
    paint,
    changed: () => { ports.changed(); paint(); },
  };
}

/** NapCat 的端口：写 onebot.listen，写错的照网页交回的原话说。存好了桥收到推送当场换，再照 status 看实际听的（「施工时定的」
 *  第 156 条）。交回 `{el, changed}`：设置项变了、格里没改过的跟着换。 */
function portsCard(ui) {
  const { h, say } = ui;
  const saved = () => String(ui.values()[KEYS.listen] ?? '');
  const listen = h('input', { type: 'number', min: 1024, max: 65535, inputmode: 'numeric', value: saved() });
  let shown = saved();
  const note = h('p', { class: 'note' });
  const save = h('button', { class: 'button primary', type: 'submit', text: say('save') });
  const el = h('form', { class: 'card' },
    h('h2', { text: say('ports/title') }),
    h('div', { class: 'ports' }, h('label', {}, h('span', { text: say('ports/listen') }), listen), save),
    note);
  el.addEventListener('submit', async (event) => {
    event.preventDefault();
    if (listen.value === saved()) return;
    save.disabled = true;
    note.className = 'note';
    note.textContent = '';
    const wanted = Number(listen.value);
    try {
      await ui.save([{ key: KEYS.listen, input: listen.value }]);
      shown = saved();
      for (let i = 0; i < MOVE_CHECKS; i += 1) {
        await sleep(MOVE_CHECK_MS);
        if ((await ui.refresh())?.listen === wanted) {
          note.textContent = say('ports/listen-moved', { port: wanted });
          return;
        }
      }
      note.className = 'note error';
      note.textContent = say('ports/listen-in-use', { port: wanted });
    } catch (error) {
      note.className = 'note error';
      note.textContent = say('failed', { reason: ui.reasonOf(error) });
    } finally {
      save.disabled = false;
    }
  });
  return {
    el,
    changed: () => {
      if (listen.value === shown) listen.value = saved();
      shown = saved();
    },
  };
}
