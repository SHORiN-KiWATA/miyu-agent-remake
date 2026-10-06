// @ts-check
//! 「模型」页（蓝图 `web.md`「设置页」第 14 条，照 `tui.md`「全屏配置页」）：四个标签：供应商和模型、默认文本模型、默认视觉模型、
//! 模型池。列表照 `model.list`，存照 `config.set`（和通用的行同一条路：`coreRow` 加上换成真键的项）。只放用户要看的：一家用不了的
//! 原因写一次、不在每个模型上重复；模型名只在显示名重了时写；详情里只有名字和控件。样板：添加、编辑供应商，模型池的编辑还没做。

import { h, icon, replace } from '../../src/lib/dom.js';
import { coreRow, textField } from './rows.js';
import { itemFor, plainItem, shortCount, inputText, duplicates, layerFor, writtenIn } from './model.js';

const TABS = ['providers', 'chat', 'vision', 'pools'];

/** 这一页。 */
export function drawModels(dialog) {
  const ctx = dialog.ctx;
  dialog.modelTab ??= 'providers';
  const tabs = h('div.set-tabs', { role: 'tablist' }, TABS.map((id) => h(`button.set-tabbar${dialog.modelTab === id ? '.is-on' : ''}`, {
    type: 'button',
    role: 'tab',
    onclick: () => { dialog.modelTab = id; dialog.modelDetail = null; dialog.drawBody(); },
  }, ctx.text(`models.tabs.${id}`))));
  const list = dialog.models;
  if (!list) return [tabs, h('p.set-empty.is-bad', ctx.text('load_failed', { reason: 'model.list' }))];
  if (dialog.modelTab === 'providers') return [tabs, providers(dialog, list)];
  if (dialog.modelTab === 'pools') return [tabs, pools(dialog, list)];
  return [tabs, defaults(dialog, list, dialog.modelTab)];
}

/** 模型的显示名：目录给的名字，没有的写模型名。 */
const modelName = (m) => m.facts?.name?.value ?? m.model;
/** 看得了图。 */
const seesImages = (m) => (m.facts?.inputs?.value ?? []).includes('image');
/** 这一家每个模型都一样的「用不了」（没有密钥这类）：交回它，写在供应商那一行下面一次 */
const sharedState = (models) => {
  const states = new Set(models.map((m) => m.state ?? 'ok'));
  const [only] = states;
  return states.size === 1 && only !== 'ok' && only !== 'cooling' ? only : null;
};

/** 名字，重了的下面写模型名；窗口、看得了图。 */
function modelLabel(m, dup) {
  return [
    h('span.set-model-name', modelName(m), dup.has(modelName(m)) ? h('code.set-model-id', m.model) : null),
    h('span.set-model-meta', shortCount(m.facts?.window?.value), seesImages(m) ? icon('image') : null),
  ];
}

/** 供应商和模型：左边供应商，右边它的地址、接口、密钥和模型；点模型滑进详情。 */
function providers(dialog, list) {
  const ctx = dialog.ctx;
  const all = list.providers ?? [];
  if (!all.some((p) => p.id === dialog.provider)) dialog.provider = all[0]?.id ?? null;
  const p = all.find((x) => x.id === dialog.provider);
  const side = h('div.set-prov-list',
    all.map((x) => h(`button.set-prov${x.id === dialog.provider ? '.is-on' : ''}`, { type: 'button', onclick: () => { dialog.provider = x.id; dialog.modelDetail = null; dialog.drawBody(); } },
      h('span', x.name ?? x.id), sharedState(x.models ?? []) ? h('i.set-warn-dot') : null)),
    h('button.set-prov.is-add', { type: 'button', onclick: () => dialog.toast(ctx.text('models.edit_todo')) }, icon('plus'), ctx.text('models.add_provider')));
  if (!p) return h('div.set-prov-wrap', side);
  const models = p.models ?? [];
  const key = p.keys?.[0];
  const keyText = key?.ref?.startsWith('env:') ? ctx.text('from_env', { name: key.ref.slice(4) }) : ctx.text(key?.set ? 'secret_set' : 'secret_unset');
  const address = typeof p.base_url === 'object' && p.base_url?.env ? ctx.text('from_env', { name: p.base_url.env }) : inputText(p.base_url);
  const shared = sharedState(models);
  const result = h('p.set-test', { hidden: true });
  const test = h('button.set-btn', { type: 'button', onclick: async () => {
    test.disabled = true;
    result.hidden = false;
    result.className = 'set-test';
    replace(result, icon('loader-circle'));
    try {
      const r = await ctx.core.request('provider.test', { provider: p.id, ...(dialog.modelDetail ? { model: dialog.modelDetail } : {}) });
      result.className = `set-test ${r.ok ? 'is-good' : 'is-bad'}`;
      replace(result, r.ok ? ctx.text('models.test_ok', { model: r.model, ms: r.first_token_ms }) : r.error?.message ?? r.stage);
    } catch (err) {
      result.className = 'set-test is-bad';
      replace(result, err.message);
    }
    test.disabled = false;
  } }, ctx.text('models.test'));
  const head = h('div.set-prov-facts',
    fact(ctx.text('models.address'), address),
    fact(ctx.text('models.driver'), p.driver ?? ''),
    fact(ctx.text('models.key'), keyText),
    h('div.set-prov-buttons', h('button.set-btn', { type: 'button', onclick: () => dialog.toast(ctx.text('models.edit_todo')) }, ctx.text('models.edit')), test));
  const dup = duplicates(models.map(modelName));
  const grid = models.length
    ? h('div.set-model-grid', models.map((m) => h(`button.set-model${dialog.modelDetail === m.model ? '.is-on' : ''}`, { type: 'button', onclick: () => { dialog.modelDetail = m.model; dialog.drawBody(); } },
      modelLabel(m, dup), !shared && m.state && m.state !== 'ok' ? h('em.set-model-state', ctx.text(`models.states.${m.state}`)) : null)))
    : h('p.set-empty', ctx.text('models.no_models'));
  const main = h('div.set-prov-main', head, shared ? h('p.set-prov-warn', ctx.text(`models.states.${shared}`)) : null, result, grid);
  // 详情盖着右边整栏（挂在 `.set-main` 上，滚正文时它不跟着走）
  const detail = dialog.modelDetail ? drawer(dialog, p, models.find((m) => m.model === dialog.modelDetail)) : null;
  if (detail) {
    // 存了一项重画时还是同一个模型：不再滑进来一次
    if (dialog.drawerFor === dialog.modelDetail) detail.classList.add('is-settled');
    dialog.panel.querySelector('.set-main')?.append(detail);
  }
  dialog.drawerFor = dialog.modelDetail;
  return h('div.set-prov-wrap', side, main);
}

const fact = (label, value) => h('div.set-fact', h('span', label), h('strong', { title: value }, value));

/** 一个模型的详情：从右边滑进来，只有名字和控件；`Esc`、✕ 收回。 */
function drawer(dialog, p, m) {
  if (!m) return null;
  const ctx = dialog.ctx;
  const names = { id: p.id, model: m.model };
  const f = m.facts ?? {};
  const item = (field, factOf) => itemFor(dialog.schema, dialog.got, `providers.<id>.models.<model>.${field}`, names, factOf ? { value: factOf.value } : undefined);
  const row = (field, factOf, label) => {
    const it = item(field, factOf);
    if (!it) return null;
    if (label) it.name = label;
    return coreRow(dialog, it, { compact: true });
  };
  // 思考强度：只列这个模型报的那几档，前面一个「默认」（选它是删掉这一项）
  const effort = item('effort');
  if (effort) {
    effort.name = ctx.text('models.effort');
    effort.control = 'select';
    effort.options = [{ value: null, name: ctx.text('default_mark') }, ...(f.reasoning?.value ?? []).map((v) => ({ value: v, name: v }))];
  }
  const close = () => { dialog.modelDetail = null; dialog.drawBody(); };
  const el = h('aside.set-drawer',
    h('header.set-drawer-head', h('div', h('strong', modelName(m)), h('code', m.model)), h('button.icon-button', { type: 'button', 'aria-label': ctx.text('close'), onclick: close }, icon('x'))),
    h('div.set-rows',
      row('inputs', f.inputs, ctx.text('models.inputs')),
      row('window', f.window, ctx.text('models.window')),
      effort && effort.options && effort.options.length > 1 ? coreRow(dialog, effort, { compact: true }) : null),
    prices(dialog, item));
  el.addEventListener('set-dismiss', close);
  return el;
}

/** 价格一块：输入、输出、缓存读、缓存写两行两列，币种跟在后面；空着的是没写（不当成 0）。 */
function prices(dialog, item) {
  const ctx = dialog.ctx;
  const fields = ['input', 'output', 'cache_read', 'cache_write'];
  const field = (it, kind) => textField(inputText(it.entry?.value ?? ''), kind, '', async (text) => {
    const why = await dialog.save(it, text === '' ? { unset: true } : { input: text });
    if (why) dialog.toast(why);
  });
  const cells = [];
  let changed = false;
  for (const name of [...fields, 'currency']) {
    const it = item(`price.${name}`);
    if (!it) continue;
    changed ||= writtenIn(it.entry, layerFor(it));
    cells.push(h(`label.set-price${name === 'currency' ? '.is-currency' : ''}`, h('span', ctx.text(`models.price_${name}`)), field(it, name === 'currency' ? 'text' : 'number')));
  }
  return h('section.set-prices',
    h('div.set-prices-head', h('span', ctx.text('models.price')), changed ? h('em', ctx.text('changed')) : null),
    h('div.set-price-grid', cells));
}

/** 默认文本模型、默认视觉模型：一列模型单选，选了写 `models.chat`、`models.vision`（不改开着的会话）。 */
function defaults(dialog, list, which) {
  const item = plainItem(dialog.schema, dialog.got, `models.${which}`);
  if (!item) return null;
  const current = item.entry?.value ?? null;
  const all = (list.providers ?? []).flatMap((p) => (p.models ?? []).map((m) => ({ p, m }))).filter(({ m }) => which !== 'vision' || seesImages(m));
  const dup = duplicates(all.map(({ m }) => modelName(m)));
  const pick = async (ref) => {
    if (ref === current) return;
    const why = await dialog.save(item, { value: ref });
    if (why) dialog.toast(why);
  };
  // 只有一家的不写是哪一家
  const many = (list.providers ?? []).length > 1;
  return h('div.set-picks', all.map(({ p, m }) => h(`button.set-pick${m.ref === current ? '.is-on' : ''}`, { type: 'button', onclick: () => pick(m.ref) },
    h('i.set-radio'), modelLabel(m, dup), many ? h('span.set-pick-provider', p.name ?? p.id) : null)));
}

/** 模型池：一个池一张卡片（名字、调用方式、成员），最后一张「新建模型池」。样板：只能看。 */
function pools(dialog, list) {
  const ctx = dialog.ctx;
  const cards = (list.pools ?? []).map((pool) => h('div.set-pool',
    h('div.set-pool-head', h('strong', `@${pool.name}`), h('span.set-pill', ctx.text(`models.strategies.${pool.strategy}`))),
    h('div.set-chips', (pool.models ?? []).length ? pool.models.map((ref) => h('span.set-chip', h('span', ref))) : h('span.set-muted', ctx.text('models.pool_empty')))));
  const add = h('button.set-pool.is-add', { type: 'button', onclick: () => dialog.toast(ctx.text('models.pool_todo')) }, icon('plus'), ctx.text('models.new_pool'));
  return h('div.set-pools', cards, add);
}
