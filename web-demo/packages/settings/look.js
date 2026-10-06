// @ts-check
//! 网页自己的设置（蓝图 `web.md`「设置页」第 11、12 条）：「外观」页只有主题卡片；一个包的设置项（一个个颜色、重画间隔这类）在
//! 「软件包」页点那个包展开（`packageSettings`）。存在网页这边（内核的 `packages`：出厂 → 发行版 → 个人 · 这台设备），画法和核心的
//! 项同一个 `shell`，小字只在改过时写「已改」。

import { h, icon, replace } from '../../src/lib/dom.js';
import { shell, groupBlock, toggle, textField, select } from './rows.js';

/** 一个先空着、读完再填的块（读包的设置是异步的）。 */
export function later(fill) {
  const el = h('div.set-later');
  fill(el).catch((err) => replace(el, h('p.set-empty.is-bad', err.message)));
  return el;
}

/** 「外观」页：主题卡片，第一张「跟随系统」。 */
export function drawLook(dialog) {
  const ctx = dialog.ctx;
  return later(async (el) => {
    const theme = await ctx.packages.settings('theme');
    const palettes = ctx.slots.list('theme.palettes');
    const chosen = theme.values.palette ?? null;
    const pick = async (id) => {
      await ctx.packages.set('theme', { config: { palette: id } });
      dialog.drawBody();
    };
    const card = (id, name, colors) => h(`button.set-theme${chosen === id ? '.is-on' : ''}`, { type: 'button', onclick: () => pick(id) },
      h('span.set-theme-sample', colors
        ? [h('i', { style: { background: colors.sidebar_bg } }), h('i', { style: { background: colors.surface } },
          h('b', { style: { background: colors.accent } }), h('b', { style: { background: colors.text, opacity: '0.5' } }), h('b', { style: { background: colors.gold } }))]
        : palettes.slice(0, 2).map((p) => h('i', { style: { background: p.render().page.surface } }))),
      h('span.set-theme-name', name));
    replace(el, groupBlock(ctx.text('theme'), [h('div.set-themes', card(null, ctx.text('follow_system'), null), palettes.map((p) => card(p.id, p.name, p.render().page)))]));
  });
}

/** 收起来的「高级（N）」：点开看。 */
function fold(ctx, rows) {
  if (!rows.length) return null;
  const box = h('div.set-fold', { hidden: true }, rows);
  const head = h('button.set-fold-head', { type: 'button', onclick: () => { box.hidden = !box.hidden; head.classList.toggle('is-open', !box.hidden); } },
    icon('chevron-right'), ctx.text('advanced', { count: rows.length }));
  return h('div', head, box);
}

/**
 * 一个包的设置项（「软件包」页展开时）：`advanced` 的收在最后「高级（N）」里；主题的颜色包把终端那一组（`t_` 打头）也收进去。
 * @param {any} dialog @param {string} id 包
 */
export function packageSettings(dialog, id) {
  const ctx = dialog.ctx;
  return later(async (el) => {
    const got = await ctx.packages.settings(id);
    const specs = got.manifest.settings ?? {};
    const keys = Object.keys(specs);
    const hidden = (k) => specs[k].advanced || (id.startsWith('theme-') && k.startsWith('t_'));
    const rowOf = (k) => packageRow(dialog, id, k, got, () => replace(el, packageSettings(dialog, id)));
    replace(el, h('div.set-rows', keys.filter((k) => !hidden(k)).map(rowOf)), fold(ctx, keys.filter(hidden).map(rowOf)));
  });
}

/**
 * 网页包的一项：改了写进个人那一层，↺ 删掉个人这一项；小字只在改过时写「已改」。
 * @param {any} dialog @param {string} id 包 @param {string} key @param {{manifest: any, values: any, origins: any, errors: any[]}} got
 * @param {() => void} redraw 存好了重画这个包
 */
function packageRow(dialog, id, key, got, redraw) {
  const ctx = dialog.ctx;
  const spec = got.manifest.settings[key];
  const value = got.values[key];
  const changed = got.origins[key] === 'user';
  /** @type {ReturnType<typeof shell>} */
  let row;
  const set = async (v) => {
    row.busy();
    try {
      await ctx.packages.set(id, { config: { [key]: v } });
      row.done();
      setTimeout(redraw, 300);
    } catch (err) {
      row.fail(err.message);
    }
  };
  row = shell({
    name: ctx.local(spec.name ?? key),
    description: spec.description ? ctx.local(spec.description) : '',
    source: changed ? ctx.text('changed') : null,
    problems: got.errors.filter((e) => e.key === key).map((e) => ({ level: 'error', message: e.error })),
    control: packageControl(dialog, spec, value, set),
    reset: changed ? () => set(null) : null,
    resetTitle: ctx.text('reset'),
  });
  return row.el;
}

/** 网页包设置项的控件，照清单的 `type`。结构复杂的（`json`、`map`、`list`）样板里先只读。 */
function packageControl(dialog, spec, value, set) {
  if (spec.type === 'boolean') return toggle(!!value, set);
  if (spec.type === 'choice') return select(dialog, (spec.choices ?? []).map((c) => ({ value: c, name: c })), value, set);
  if (spec.type === 'number' || spec.type === 'duration') {
    return textField(String(value), 'number', [spec.min, spec.max].some((x) => x != null) ? `${spec.min ?? ''} – ${spec.max ?? ''}` : '', (text) => {
      const n = Number(text);
      if (Number.isFinite(n)) set(n);
    });
  }
  if (spec.type === 'color') {
    const hex = typeof value === 'string' && /^#[0-9a-f]{6}$/i.test(value);
    const field = textField(String(value), 'text', '', (text) => set(text));
    if (!hex) return field;
    const swatch = h('input.set-color', { type: 'color', value, onchange: (e) => set(e.target.value) });
    return h('div.set-color-row', swatch, field);
  }
  if (spec.type === 'text' || spec.type === 'key') return textField(String(value ?? ''), 'text', '', set);
  return h('code.set-json', { title: JSON.stringify(value) }, JSON.stringify(value));
}
