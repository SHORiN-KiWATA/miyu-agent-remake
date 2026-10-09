// @ts-check
//! 「模型」页（蓝图 `web.md`「设置页」第 14 条，照 `tui.md`「全屏配置页」）：四个标签：供应商和模型、默认文本模型、默认视觉模型、
//! 模型池。列表照 `model.list`，存照 `config.set`（和通用的行同一条路：`coreRow` 加上换成真键的项）。只放用户要看的：一家用不了的
//! 原因写一次、不在每个模型上重复；模型名只在显示名重了时写；详情里只有名字和控件。添加、编辑供应商的表在 `provider-form.js`，
//! 模型池在 `pools.js`。

import { h, icon, replace } from '../../src/lib/dom.js';
import { coreRow, textField } from './rows.js';
import { itemFor, plainItem, shortCount, inputText, duplicates, layerFor, writtenIn, expectFor, providerName, filterModels } from './model.js';
import { providerForm, openForm } from './provider-form.js';
import { drawPools } from './pools.js';
import { drawEmbedding } from './embedding.js';

const TABS = ['providers', 'chat', 'vision', 'embedding', 'pools'];

/** 这一页。 */
export function drawModels(dialog) {
  const ctx = dialog.ctx;
  dialog.modelTab ??= 'providers';
  // 「语义模型」（`models.embedding`，2026-10-09 项目主人定名），旧核心没有这一项的不出这个标签
  const embed = plainItem(dialog.schema, dialog.got, 'models.embedding');
  const shownTabs = TABS.filter((id) => id !== 'embedding' || embed);
  if (!shownTabs.includes(dialog.modelTab)) dialog.modelTab = 'providers';
  const tabs = h('div.set-tabs', { role: 'tablist' }, shownTabs.map((id) => h(`button.set-tabbar${dialog.modelTab === id ? '.is-on' : ''}`, {
    type: 'button',
    role: 'tab',
    onclick: () => { dialog.modelTab = id; dialog.modelDetail = null; dialog.providerForm = null; dialog.drawBody(); },
  }, ctx.text(`models.tabs.${id}`))));
  const list = dialog.models;
  if (!list) return [tabs, dialog.modelsLoading ? loadingRow(ctx) : h('p.set-empty.is-bad', ctx.text('load_failed', { reason: 'model.list' }))];
  if (dialog.modelTab === 'providers') return [tabs, dialog.providerForm ? providerForm(dialog, list.providers ?? []) : providers(dialog, list)];
  if (dialog.modelTab === 'pools') return [tabs, drawPools(dialog, list)];
  if (dialog.modelTab === 'embedding') return [tabs, drawEmbedding(dialog)];
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
  // 记着的那一家（刚建好、模型列表还没读回来的）不在表里时先看第一家，不改记着的
  const p = all.find((x) => x.id === dialog.provider) ?? all[0];
  const side = h('div.set-prov-list',
    all.map((x) => h(`button.set-prov${x.id === p?.id ? '.is-on' : ''}`, { type: 'button', onclick: () => { dialog.provider = x.id; dialog.modelDetail = null; if (dialog.modelQuery) dialog.modelQuery.providers = ''; dialog.drawBody(); } },
      h('span', providerName(x)), sharedState(x.models ?? []) ? h('i.set-warn-dot') : null)),
    h('button.set-prov.is-add', { type: 'button', onclick: openForm(dialog, null) }, icon('plus'), ctx.text('models.add_provider')));
  if (!p) return h('div.set-prov-wrap', side);
  const models = p.models ?? [];
  // 一家一个 key（核心 8-25）：`model.list` 每家的 `key`，没写的没有这一格
  const key = p.key;
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
      // 测成了：核心顺手存了供应商列出来的模型，重读这一家
      if (r.ok) refreshProvider(dialog, p.id, false);
    } catch (err) {
      result.className = 'set-test is-bad';
      replace(result, err.message);
    }
    test.disabled = false;
  } }, ctx.text('models.test'));
  const head = h('div.set-prov-facts',
    fact(ctx.text('models.address'), address),
    // 核心推不出接口、先照 openai-chat 用的（`driver_from: default`，核心 8-26）：写明是猜的，猜错了请求会出错
    fact(ctx.text('models.driver'), p.driver ?? '', p.driver_from === 'default' ? ctx.text('models.driver_guess') : null),
    fact(ctx.text('models.key'), keyText),
    h('div.set-prov-buttons', h('button.set-btn', { type: 'button', onclick: openForm(dialog, p.id) }, ctx.text('models.edit')), test));
  // 还没有模型的一家：请核心去拉一次（这个弹窗里每家一次），拉的时候写「正在读模型列表」
  if (!models.length && !dialog.refreshed?.has(p.id)) queueMicrotask(() => refreshProvider(dialog, p.id, true));
  const dup = duplicates(models.map(modelName));
  const rows = models.map((m) => ({ name: modelName(m), model: m.model, node: h(`button.set-model${dialog.modelDetail === m.model ? '.is-on' : ''}`, { type: 'button', onclick: () => { dialog.modelDetail = m.model; dialog.drawBody(); } },
    modelLabel(m, dup), !shared && m.state && m.state !== 'ok' ? h('em.set-model-state', ctx.text(`models.states.${m.state}`)) : null) }));
  let grid;
  if (models.length) {
    const find = searchable(dialog, 'providers', rows);
    grid = [find.input, h('div.set-model-grid', rows.map((r) => r.node)), find.empty];
  } else grid = dialog.loadingProvider === p.id || dialog.modelsLoading ? loadingRow(ctx) : h('p.set-empty', ctx.text('models.no_models'));
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

const fact = (label, value, note = null) => h('div.set-fact', h('span', label), h('strong', { title: value }, value, note ? h('em.set-fact-note', note) : null));

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
      effort && effort.options && effort.options.length > 1 ? coreRow(dialog, effort, { compact: true }) : null,
      // 默认温度（核心 8-22）：这个模型收温度的才有这一行；空着是不带，照供应商的默认
      f.takes_temperature?.value !== false ? row('temperature', undefined, ctx.text('models.temperature')) : null),
    prices(dialog, item));
  el.addEventListener('set-dismiss', close);
  return el;
}

/** 价格一块：输入、输出、缓存读、缓存写两行两列，币种跟在后面；改一项时五项照显示的整份存（照终端第 4 条），空着的是删掉（不当成 0）。 */
function prices(dialog, item) {
  const ctx = dialog.ctx;
  const parts = ['input', 'output', 'cache_read', 'cache_write', 'currency'].map((name) => ({ name, it: item(`price.${name}`) })).filter((x) => x.it);
  /** @type {Map<string, HTMLInputElement>} */
  const inputs = new Map();
  // 照显示的整份写：有字的都写（没改的也写，五项一起成一套），空着的这一层写过才删
  const saveAll = async () => {
    const layer = layerFor(parts[0].it);
    const changes = parts.flatMap(({ name, it }) => {
      const text = inputs.get(name)?.value.trim() ?? '';
      const expect = expectFor(it.entry, layer);
      if (text) return [{ key: it.key, input: text, expect }];
      return writtenIn(it.entry, layer) ? [{ key: it.key, unset: true, expect }] : [];
    });
    const why = await dialog.saveMany(layer, changes);
    if (why) dialog.toast(why);
  };
  let changed = false;
  const cells = parts.map(({ name, it }) => {
    changed ||= writtenIn(it.entry, layerFor(it));
    const field = /** @type {HTMLInputElement} */ (textField(inputText(it.entry?.value ?? ''), name === 'currency' ? 'text' : 'number', '', saveAll));
    inputs.set(name, field);
    return h(`label.set-price${name === 'currency' ? '.is-currency' : ''}`, h('span', ctx.text(`models.price_${name}`)), field);
  });
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
  const rows = all.map(({ p, m }) => ({ name: modelName(m), model: m.model, provider: providerName(p), node: h(`button.set-pick${m.ref === current ? '.is-on' : ''}`, { type: 'button', onclick: () => pick(m.ref) },
    h('i.set-radio'), modelLabel(m, dup), many ? h('span.set-pick-provider', providerName(p)) : null) }));
  const find = searchable(dialog, which, rows);
  return [find.input, h('div.set-picks', rows.map((r) => r.node)), find.empty];
}

/** 「正在读模型列表」那一行，前面一个转圈。 */
const loadingRow = (ctx) => h('p.set-loading', icon('loader-circle'), ctx.text('models.loading'));

/**
 * 搜索框：照显示名、模型名、供应商名就地藏掉对不上的行（不重画，打字不丢焦点）；字记在弹窗上，重画以后照旧。
 * @param {any} dialog @param {string} slot 记在哪一格（每个标签一格） @param {{name: string, model: string, provider?: string, node: HTMLElement}[]} rows
 */
function searchable(dialog, slot, rows) {
  dialog.modelQuery ??= {};
  const input = /** @type {HTMLInputElement} */ (h('input.set-input.set-model-search', { type: 'search', placeholder: dialog.ctx.text('models.search'), value: dialog.modelQuery[slot] ?? '', spellcheck: 'false', autocomplete: 'off' }));
  const empty = h('p.set-empty', { hidden: true }, dialog.ctx.text('models.no_match'));
  const apply = () => {
    dialog.modelQuery[slot] = input.value;
    const shown = new Set(filterModels(rows, input.value));
    for (const r of rows) r.node.hidden = !shown.has(r);
    empty.hidden = shown.size > 0;
  };
  input.addEventListener('input', apply);
  apply();
  return { input, empty };
}

/**
 * 重读一家的模型（`refresh` 的请核心先去供应商那边拉一遍）：读的时候那一块写「正在读模型列表」，读完换上、重画。
 * @param {any} dialog @param {string} id @param {boolean} refresh
 */
export async function refreshProvider(dialog, id, refresh) {
  dialog.refreshed ??= new Set();
  dialog.refreshed.add(id);
  dialog.loadingProvider = id;
  if (dialog.isOpen) dialog.drawBody();
  try {
    const got = await dialog.ctx.core.request('model.list', { provider: id, ...(refresh ? { refresh: true } : {}) });
    const fresh = got?.providers?.find((x) => x.id === id);
    if (fresh && dialog.models) dialog.models.providers = dialog.models.providers.map((x) => (x.id === id ? fresh : x));
  } catch {
    // 读不到的照旧显示原来的（没有模型的写「还没有模型」）
  } finally {
    dialog.loadingProvider = null;
    if (dialog.isOpen && !dialog.editing()) dialog.drawBody();
  }
}
