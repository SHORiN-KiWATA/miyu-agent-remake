// 接入QQ 后台页的「白名单成员」页（docs/blueprint/onebot.md 第一条「后台页」第 3 条，施工 O-28 上）：一张能加、能删、能存的表。
//
// - 设置项 onebot.whitelist 是平台身份的列表，页面只画、只改这个桥的平台（status 的 platform）的，一行一个号；整张写回，别的平台
//   的身份照原样留在前面（「后台页」第 3 条「白名单成员」）。
// - 号去掉前后空白，照 1 到 9 开头、一共 1 到 20 位数字认；空着的行不算；不对的、重复的标出来，有标着的不让存（同上）。页面
//   的校验只是让人早点看见，算数的是核心：网页交回的问题照原话说在表下面，表里的东西不丢。
// - 终端管理员不在这页：在网页的「高级」页改（「施工时定的」第 164 条），页上说一句。
//
// 骨架、字、通道都在 app.js，经 whitelistView 的参数交过来；这里不碰全局的东西。

/** 白名单成员那一项的键。 */
const WHITELIST = 'onebot.whitelist';

/** 号：1 到 9 开头，一共 1 到 20 位数字。桥照整数拼 `qq:<号>`，以 0 开头的写进去永远对不上。 */
const NUMBER = /^[1-9][0-9]{0,19}$/;

/** 平台身份 `<platform>:<号>` 里的号；不是这个平台的交回 null。 */
function numberOf(identity, platform) {
  const prefix = `${platform}:`;
  return typeof identity === 'string' && identity.startsWith(prefix) ? identity.slice(prefix.length) : null;
}

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

/** 要发的改动：整张写回，别的平台的照原样在前面；和存着的 `saved` 一样的没有改动。 */
function changesOf(saved, numbers, platform) {
  const others = saved.filter((identity) => numberOf(identity, platform) == null);
  const value = [...others, ...numbers.map((number) => `${platform}:${number}`)];
  const same = value.length === saved.length && value.every((identity, at) => identity === saved[at]);
  return same ? [] : [{ key: WHITELIST, value }];
}

/** 「白名单成员」页。`ui` 同 connection.js（用 `h`、`say`、`reasonOf`、`values()`、`status()`、`save(changes)`）。平台的名字照
 *  status 的 platform，还没调到 status 的先说「正在连」，调到了再画表。页名不另写一行：页签就是（「施工时定的」第 163 条）。
 *  交回 `{el, paint, changed}`。 */
export function whitelistView(ui) {
  const { h, say } = ui;
  const body = h('div', {}, h('p', { class: 'boot-wait', text: say('loading') }));
  let table = null;
  return {
    el: [body],
    paint: () => {
      const platform = ui.status()?.platform;
      if (table || !platform) return;
      table = editable(ui, platform);
      body.replaceChildren(table.el);
    },
    changed: () => table?.changed(),
  };
}

/** 这个平台 `platform` 的那张表：交回卡片和设置项变了时调的 `changed`（没改动的照新的重画，有没存的改动的留着，「施工时定的」
 *  第 161 条）。 */
function editable(ui, platform) {
  const { h, say } = ui;
  const saved = () => (Array.isArray(ui.values()[WHITELIST]) ? ui.values()[WHITELIST] : []);
  const list = h('div', { class: 'people-rows' });
  const empty = h('p', { class: 'empty', text: say('whitelist/empty') });
  const note = h('p', { class: 'note' });
  const save = h('button', { class: 'button primary', type: 'button', text: say('save') });
  let rows = [];
  let busy = false;
  // 上一次照着画的那几个号：设置项变了时，照它看人改没改过（这时 values() 已经是新的；只比号，存着的先后可能和写回的不一样）。
  let drawn = '';

  /** 填着的号：去掉前后空白，空着的行不算。 */
  const filled = () => rows.map((one) => one.input.value.trim()).filter((number) => number !== '');
  const changes = () => changesOf(saved(), filled(), platform);

  /** 重标每一行的毛病，空表那一句在不在，「保存」灰不灰（没改动、有标着的、正在存）。 */
  function refresh() {
    const found = marks(rows.map((one) => one.input.value.trim()));
    rows.forEach((one, at) => {
      const mark = found[at];
      if (mark) one.input.setAttribute('aria-invalid', 'true');
      else one.input.removeAttribute('aria-invalid');
      one.aside.textContent = mark ? say(`whitelist/${mark}`) : '';
    });
    empty.hidden = rows.length > 0;
    save.disabled = busy || found.some(Boolean) || changes().length === 0;
  }

  /** 表里改了：上一次存好、没成的话收起，重标。 */
  function changed() {
    note.textContent = '';
    refresh();
  }

  /** 加一行，号的初值是 `number`；交回这一行。 */
  function add(number) {
    const input = h('input', {
      inputmode: 'numeric', autocomplete: 'off', value: number,
      placeholder: say('whitelist/number'), 'aria-label': say('whitelist/number'),
    });
    const aside = h('span', { class: 'aside error' });
    const one = { input, aside };
    const remove = () => {
      rows = rows.filter((other) => other !== one);
      one.el.remove();
      changed();
    };
    one.el = h('div', { class: 'people-row' }, input,
      h('button', { class: 'button', type: 'button', text: say('whitelist/remove'), onclick: remove }), aside);
    input.addEventListener('input', changed);
    rows.push(one);
    list.append(one.el);
    return one;
  }

  /** 照存着的设置项重画这一张表。 */
  function reset() {
    rows = [];
    list.replaceChildren();
    for (const identity of saved()) {
      const number = numberOf(identity, platform);
      if (number != null) add(number);
    }
    drawn = filled().join('\n');
    refresh();
  }

  save.addEventListener('click', async () => {
    const wanted = changes();
    if (!wanted.length) return;
    busy = true;
    note.className = 'note';
    note.textContent = '';
    refresh();
    try {
      await ui.save(wanted);
      reset();
      note.textContent = say('saved');
    } catch (error) {
      // 网页交回的问题照原话说，表里的东西不动，人改了再存。
      note.className = 'note error';
      note.textContent = say('failed', { reason: ui.reasonOf(error) });
    } finally {
      busy = false;
      refresh();
    }
  });
  const more = h('button', {
    class: 'button', type: 'button', text: say('whitelist/add'),
    onclick: () => { add('').input.focus(); changed(); },
  });
  const el = h('section', { class: 'card' },
    h('h2', { text: say('whitelist/title') }),
    h('p', { class: 'hint', text: say('whitelist/hint') }),
    empty, list,
    h('div', { class: 'people-actions' }, more, save),
    note,
    h('p', { class: 'aside-note', text: say('whitelist/admins') }));
  reset();
  return {
    el,
    changed: () => {
      if (!busy && filled().join('\n') === drawn) reset();
      else refresh();
    },
  };
}
