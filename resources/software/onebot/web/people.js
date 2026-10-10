// QQ 桥 WebUI 的「终端管理员与白名单成员」页（docs/blueprint/onebot.md 第二条「怎么走」第 4 条，施工 O-17）：两张表，各有一个「保存」。
//
// - 终端管理员：系统配置的 external.bindings，一格是一个平台身份对一个本机账号。页面只画、只改这个桥的平台（/status 的
//   platform）的，别的平台的不碰。存的时候照改动发一条 config.set：加的、改了账号的写值，删的恢复默认（unset）。
// - 白名单成员：系统配置的 onebot.trusted，平台身份的列表，整张写回；别的平台的身份照原样留在前面。
// - 号去掉前后空白，照 1 到 9 开头、一共 1 到 20 位数字认；空着的行不算；不对的、重复的标出来，有标着的不让存（「施工时
//   定的」第 28 条）。页面的校验只是让人早点看见，算数的是核心：核心回的问题照原话说在那张表下面，表里的东西不丢。
//
// 骨架、字、核心连接都在 app.js，经 peoplePage 的参数交过来；这里不碰全局的东西。

/** 终端管理员对应表的键的前缀、白名单成员那一项的键（config.md 配置项表）。 */
const BINDINGS = 'external.bindings.';
const WHITELIST = 'onebot.trusted';

/** 号：1 到 9 开头，一共 1 到 20 位数字。桥照整数拼 `qq:<号>`，以 0 开头的写进去永远对不上。 */
const NUMBER = /^[1-9][0-9]{0,19}$/;

/** 平台身份 `<platform>:<号>` 里的号；不是这个平台的交回 null。 */
function numberOf(identity, platform) {
  const prefix = `${platform}:`;
  return identity.startsWith(prefix) ? identity.slice(prefix.length) : null;
}

/** 终端管理员对应表里一格的键：`external.bindings."qq:10001"`。带冒号的那一段照核心的键的写法写成带双引号的字。 */
const bindingKey = (platform, number) => BINDINGS + JSON.stringify(`${platform}:${number}`);

/** 配置 `items`（config.get 的）里这个平台的终端管理员：号到账号。键的那一段读不懂的跳过。 */
function savedAdmins(items, platform) {
  const admins = new Map();
  for (const [key, item] of Object.entries(items)) {
    if (!key.startsWith(BINDINGS)) continue;
    let identity = key.slice(BINDINGS.length);
    try {
      if (identity.startsWith('"')) identity = JSON.parse(identity);
    } catch { continue; }
    const number = numberOf(identity, platform);
    if (number != null) admins.set(number, item.value);
  }
  return admins;
}

/** 配置 `items` 里的白名单成员，整张（别的平台的也在）；没写的是空表。 */
const savedWhitelist = (items) => items[WHITELIST]?.value ?? [];

/** 每一行号的毛病：空着的、对的是 ''，不是号的 'bad-number'，同一张表里重复的 'duplicate'（几行都标）。 */
function marks(numbers) {
  const seen = new Map();
  for (const number of numbers) seen.set(number, (seen.get(number) ?? 0) + 1);
  return numbers.map((number) => {
    if (number === '') return '';
    if (!NUMBER.test(number)) return 'bad-number';
    return seen.get(number) > 1 ? 'duplicate' : '';
  });
}

/** 终端管理员表要发的改动：`rows` 是填着的几行 `{number, account}`，`saved` 是存着的号到账号。 */
function adminChanges(saved, rows, platform) {
  const changes = rows
    .filter(({ number, account }) => saved.get(number) !== account)
    .map(({ number, account }) => ({ key: bindingKey(platform, number), value: account }));
  const kept = new Set(rows.map(({ number }) => number));
  for (const number of saved.keys()) {
    if (!kept.has(number)) changes.push({ key: bindingKey(platform, number), unset: true });
  }
  return changes;
}

/** 白名单成员要发的改动：整张写回，别的平台的照原样在前面；和存着的 `saved` 一样的没有改动。 */
function whitelistChanges(saved, numbers, platform) {
  const others = saved.filter((identity) => numberOf(identity, platform) == null);
  const value = [...others, ...numbers.map((number) => `${platform}:${number}`)];
  const same = value.length === saved.length && value.every((identity, at) => identity === saved[at]);
  return same ? [] : [{ key: WHITELIST, value }];
}

/** 一张能加、能删、能存的表。`spec`：
 *  - `name`：字的编号里的那一段（admins、whitelist）；
 *  - `rows()`：照存着的配置，一行行的初值 `{number, account?}`；
 *  - `cells(start, changed)`：号后面多的格（终端管理员的账号下拉），交回 [元素, 读出多的那几格的函数]，改了调 `changed`；
 *  - `aside(number)`：号没毛病时旁边说的一句（白名单成员的「已经是终端管理员」），没有的是空的；
 *  - `changes(rows)`：照填着的几行要发的改动，没改动的是空的；
 *  - `head`、`before`、`after`：表头、提示下面、表下面多的；`changed()`：表里改了、存好了告诉谁。
 *  交回卡片、重标一遍的 `refresh`、表里现在填着的号 `numbers`。 */
function editable(ui, spec) {
  const { h, say } = ui;
  const list = h('div', { class: 'people-rows' });
  const empty = h('p', { class: 'empty', text: say(`web/people/${spec.name}/empty`) });
  const note = h('p', { class: 'note' });
  const save = h('button', { class: 'button primary', type: 'button', text: say('web/save') });
  let rows = [];
  let busy = false;

  /** 填着的几行：号去掉前后空白，空着的行不算。 */
  const filled = () => rows.map((one) => one.read()).filter(({ number }) => number !== '');

  /** 重标每一行的毛病和旁边那一句，空表那一句、表头在不在，「保存」灰不灰（没改动、有标着的、正在存）。 */
  function refresh() {
    const found = marks(rows.map((one) => one.read().number));
    rows.forEach((one, at) => {
      const mark = found[at];
      if (mark) one.input.setAttribute('aria-invalid', 'true');
      else one.input.removeAttribute('aria-invalid');
      one.aside.textContent = mark ? say(`web/people/${mark}`) : (spec.aside?.(one.read().number) ?? '');
      one.aside.classList.toggle('error', mark !== '');
    });
    empty.hidden = rows.length > 0;
    if (spec.head) spec.head.hidden = rows.length === 0;
    save.disabled = busy || found.some(Boolean) || spec.changes(filled()).length === 0;
  }

  /** 表里改了：上一次存好、没成的话收起，重标，告诉 `spec.changed`。 */
  function changed() {
    note.textContent = '';
    refresh();
    spec.changed?.();
  }

  /** 加一行，初值是 `start`；交回这一行。 */
  function add(start) {
    const input = h('input', {
      inputmode: 'numeric', autocomplete: 'off', value: start.number ?? '',
      placeholder: say('web/people/number'), 'aria-label': say('web/people/number'),
    });
    const [cells, extra] = spec.cells?.(start, changed) ?? [null, () => ({})];
    const aside = h('span', { class: 'aside' });
    const one = { input, aside, read: () => ({ ...extra(), number: input.value.trim() }) };
    const remove = () => {
      rows = rows.filter((other) => other !== one);
      one.el.remove();
      changed();
    };
    one.el = h('div', { class: 'people-row' }, input, cells,
      h('button', { class: 'button', type: 'button', text: say('web/people/remove'), onclick: remove }), aside);
    input.addEventListener('input', changed);
    rows.push(one);
    list.append(one.el);
    return one;
  }

  /** 照存着的配置重画这一张表。 */
  function reset() {
    rows = [];
    list.replaceChildren();
    for (const start of spec.rows()) add(start);
    refresh();
  }

  save.addEventListener('click', async () => {
    const changes = spec.changes(filled());
    if (!changes.length) return;
    busy = true;
    note.className = 'note';
    note.textContent = '';
    refresh();
    try {
      await ui.save(changes);
      reset();
      note.textContent = say('web/saved');
      spec.changed?.();
    } catch (error) {
      // 核心回的问题照原话说，表里的东西不动，人改了再存。
      note.className = 'note error';
      note.textContent = ui.failed(error);
    } finally {
      busy = false;
      refresh();
    }
  });
  const more = h('button', {
    class: 'button', type: 'button', text: say('web/people/add'),
    onclick: () => { add({}).input.focus(); changed(); },
  });
  const el = h('section', { class: `card people-${spec.name}` },
    h('h2', { text: say(`web/people/${spec.name}/title`) }),
    h('p', { class: 'hint', text: say(`web/people/${spec.name}/hint`) }),
    spec.before, spec.head, empty, list,
    h('div', { class: 'people-actions' }, more, save),
    note, spec.after);
  reset();
  return { el, refresh, numbers: () => filled().map(({ number }) => number) };
}

/** 「终端管理员与白名单成员」页的两张卡片。`ui`：`h`、`say`（app.js 的），`platform`（/status 的），`account`（握手回的账号），
 *  `items()`（这时的配置），`save(changes)`（config.set 写系统配置、重读配置），`failed(error)`（核心拒绝的原话）。 */
export function peoplePage(ui) {
  const { h, say, platform } = ui;
  let whitelist = null;
  const admins = editable(ui, {
    name: 'admins',
    rows: () => [...savedAdmins(ui.items(), platform)].map(([number, account]) => ({ number, account })),
    // 账号的下拉：协议里没有列账号的方法，下拉里是握手回的账号，加上表里已经写着的（手写的别的账号照原样留着）。
    cells: (start, changed) => {
      const names = new Set([ui.account, ...savedAdmins(ui.items(), platform).values(), start.account]);
      const select = h('select', { 'aria-label': say('web/people/account') },
        [...names].filter(Boolean).map((name) => h('option', { value: name, text: name })));
      select.value = start.account ?? ui.account;
      select.addEventListener('change', changed);
      return [select, () => ({ account: select.value })];
    },
    changes: (rows) => adminChanges(savedAdmins(ui.items(), platform), rows, platform),
    changed: () => whitelist?.refresh(),
    head: h('div', { class: 'people-head' },
      h('span', { text: say('web/people/number') }),
      h('span', { text: say('web/people/account') })),
    after: h('details', { class: 'risk' },
      h('summary', { text: say('web/people/risk/title') }),
      h('ul', {}, ['stolen', 'forged', 'injection'].map((key) => h('li', { text: say(`web/people/risk/${key}`) })))),
  });
  whitelist = editable(ui, {
    name: 'whitelist',
    rows: () => savedWhitelist(ui.items())
      .map((identity) => numberOf(identity, platform))
      .filter((number) => number != null)
      .map((number) => ({ number })),
    changes: (rows) => whitelistChanges(savedWhitelist(ui.items()), rows.map(({ number }) => number), platform),
    // 终端管理员的权限包含白名单成员的：照样能存，只说一句（施工单「要定的」第 3 条）。
    aside: (number) => (admins.numbers().includes(number) ? say('web/people/already-admin') : ''),
    before: h('p', { class: 'hint', text: say('web/people/whitelist/later') }),
  });
  return [admins.el, whitelist.el];
}
