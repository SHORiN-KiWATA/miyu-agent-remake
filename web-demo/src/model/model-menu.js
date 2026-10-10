// @ts-check
//! 换模型的菜单要列什么（蓝图 `web.md`「换模型的菜单」，照 Claude 网页端的模型菜单）：照核心的 `model.list`（核心施工 8-7、
//! 8-8、8-9）排出模型、模型池两页；框下面那一截怎么拆。纯函数，画在 `ui/model-menu.js`。

import { res, t } from '../util/res.js';
import { hhmm } from './format.js';

/**
 * @typedef {{ref: string, title: string, desc: string, current: boolean, usable: boolean, why: string, find: string}} Row 一行：选了交给
 *   核心的引用、上面一行、下面一行小字、是不是现在用着的、能不能选、不能选为什么（悬停写）、搜索照着找的字（显示名、模型名、供应商）
 */

/**
 * @param {any} list `model.list` 的回应；还没有的是 `null`
 * @param {string|null} current 会话现在的引用（选过还没生效的照选的）
 * @returns {{models: Row[], pools: Row[]}}
 */
export function menuOf(list, current) {
  if (!list) return { models: [], pools: [] };
  // 上面写显示名（目录里没有的写模型名），下面写供应商的显示名；显示名重了的（同一个模型的几条线路）下面接模型名分开它们
  const all = (list.providers ?? []).flatMap((p) => (p.models ?? []).map((m) => ({ p, m, name: m.facts?.name?.value || m.model, provider: providerName(p) })));
  const seen = new Map();
  for (const x of all) seen.set(x.name, (seen.get(x.name) ?? 0) + 1);
  const models = all.map(({ m, name, provider }) => {
    const usable = m.state === 'ok';
    const desc = (seen.get(name) ?? 0) > 1 && name !== m.model ? `${provider} · ${m.model}` : provider;
    return { ref: m.ref, title: name, desc, current: m.ref === current, usable, why: usable ? '' : why(m), find: `${name}\n${m.model}\n${provider}` };
  });
  const pools = (list.pools ?? []).map((pool) => {
    const ref = `@${pool.name}`;
    const members = pool.models.map((r) => footerOf(r).model).join(t('list_sep'));
    // 轮流用的写一句；出错了换下一个是池本来的样子，不写（2026-10-10 项目主人：「出错时切换」没必要）
    const how = pool.strategy === 'rotate' ? t('model_menu.rotate') : '';
    // 成员是空的（下架的模型被核心拿空了，核心 8-23 的 `usable`）：暗着选不了，照它挑会是 `no_model`
    const usable = pool.usable !== false && pool.models.length > 0;
    return { ref, title: ref, desc: [how, members].filter(Boolean).join(' · '), current: ref === current, usable, why: usable ? '' : t('model_menu.pool_empty'), find: `${ref}\n${members}` };
  });
  return { models, pools };
}

/** 供应商的显示名（核心 8-21：`name.value`，已经照配置、目录、编号退好；旧核心是字或没有）。 */
const providerName = (p) => (typeof p.name === 'object' ? p.name?.value : p.name) || p.id;

/**
 * 搜索框：照 `find`（显示名、模型名、供应商）找，空格分开的几段都要有，不分大小写；空的全给。
 * @param {Row[]} rows @param {string} query
 */
export function filterRows(rows, query) {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  return words.length ? rows.filter((r) => words.every((w) => r.find.toLowerCase().includes(w))) : rows;
}

/** 用不了的为什么：冷却到几点（本地时间）、没设 key。 */
function why(m) {
  if (m.state === 'cooling') return t('model_menu.cooling', { time: m.until ? hhmm(m.until) : '' });
  return t('model_menu.no_key');
}

/**
 * 框下面那一截：引用照第一个 `/` 拆成模型名和供应商（模型名里可以带 `/`）；池照原样写，不写供应商。
 * @param {string} ref
 * @returns {{model: string, endpoint: string|null}}
 */
export function footerOf(ref) {
  const cut = ref.startsWith('@') ? -1 : ref.indexOf('/');
  return cut > 0 ? { model: ref.slice(cut + 1), endpoint: ref.slice(0, cut) } : { model: ref, endpoint: null };
}

/**
 * 框下面那一截给人看的样子（蓝图「换模型的菜单」第 1 条）：模型写显示名（`facts.name`；重名的也写显示名，这一截只有一行），
 * 列表里找不到的写模型名；供应商写它的显示名；`full` 是悬停写的完整 `供应商/模型`。池（`@名字`）、还没有列表的照原样。
 * @param {{model: string|null, endpoint: string|null}} left 照引用、核心报的拆出来的模型名和供应商编号
 * @param {any} list `model.list` 的回应
 * @returns {{model: string|null, endpoint: string|null, full?: string}}
 */
export function footerLabel(left, list) {
  if (!list || !left.model || !left.endpoint || left.model.startsWith('@')) return left;
  const p = (list.providers ?? []).find((x) => x.id === left.endpoint);
  if (!p) return left;
  const m = p.models?.find((x) => x.model === left.model);
  return { model: m?.facts?.name?.value || left.model, endpoint: providerName(p), full: `${left.endpoint}/${left.model}` };
}

/**
 * 这个模型的思考强度有哪几档（`model.list` 里它的 `facts.reasoning`，核心施工 8-7；能关的带 `off`，8-18）。池、列表里没有的、
 * 没写的是空的：菜单里就不画思考强度那一行（池不设思考强度，蓝图「换模型的菜单」第 2 条）。
 * @param {any} list
 * @param {string|null} ref
 * @returns {string[]}
 */
export function effortLevels(list, ref) {
  if (!list || !ref || ref.startsWith('@')) return [];
  for (const p of list.providers ?? []) {
    const m = (p.models ?? []).find((x) => x.ref === ref);
    if (m) return Array.isArray(m.facts?.reasoning?.value) ? m.facts.reasoning.value : [];
  }
  return [];
}

/**
 * 这个模型现在那一档和它在配置里的键名（`model.list` 里它的 `facts.effort` `{value, from, layer, key}`，核心施工 8-18 补）：个人设置里写了的
 * （`from` 是 `config`、`layer` 是 `personal`）照写；系统配置的、没写的就是默认（`null`，框下面不写、菜单里勾默认）。池、列表里没有的、核心没给键名的
 * `key` 是 `null`：选不了。
 * @param {any} list
 * @param {string|null} ref
 * @returns {{level: string|null, key: string|null}}
 */
export function effortOf(list, ref) {
  if (!list || !ref || ref.startsWith('@')) return { level: null, key: null };
  for (const p of list.providers ?? []) {
    const e = (p.models ?? []).find((x) => x.ref === ref)?.facts?.effort;
    if (e) return { level: e.from === 'config' && e.layer === 'personal' ? e.value ?? null : null, key: typeof e.key === 'string' ? e.key : null };
  }
  return { level: null, key: null };
}

/**
 * 选了一档写进个人设置的 `config.set` 参数（蓝图「换模型的菜单」第 2 条）：选默认（`null`）的删掉个人这一项。
 * @param {string} key 完整键名，照抄核心给的
 * @param {string|null} level
 */
export function effortChange(key, level) {
  return { layer: 'personal', changes: [level === null ? { key, unset: true } : { key, value: level }] };
}

/**
 * 手动选的模型记成新会话的默认（蓝图「换模型的菜单」第 5 条，2026-10-02 项目主人定，照 `models.md`「头的约定」）：
 * 写个人设置的 `models.chat` 的 `config.set` 参数。
 * @param {string} ref 模型或 `@池`
 */
export function defaultModelChange(ref) {
  return { layer: 'personal', changes: [{ key: 'models.chat', value: ref }] };
}

/** 一档怎么写：`null` 是默认；表里没有的照原样。 @param {string|null} level */
export function effortLabel(level) {
  return level === null ? t('model_menu.effort.default') : res.text.model_menu?.effort?.levels?.[level] ?? level;
}

/**
 * 思考强度的子菜单（蓝图「换模型的菜单」第 2 条）：默认一直有，别的只列这个模型报的那几档（没有的不列，2026-10-02 项目主人定），
 * 照 `layout.json` 的 `effort_order` 排（关、默认、极低、低、中、高、更高、最高；只有开关的是 关、默认、开）。认不出的名字照原样
 * 排在后面。现在那一档打勾（`null` 是默认）。
 * @param {string[]} levels 这个模型报的几档
 * @param {string|null} current
 * @returns {{level: string|null, title: string, current: boolean}[]}
 */
export function effortRows(levels, current) {
  const order = /** @type {(string|null)[]} */ (res.layout.effort_order);
  const known = order.filter((lv) => lv === null || levels.includes(lv));
  const rest = levels.filter((lv) => !order.includes(lv));
  return [...known, ...rest].map((level) => ({ level, title: effortLabel(level), current: level === current }));
}
