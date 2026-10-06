// @ts-check
//! 设置页的纯逻辑（蓝图 `web.md`「设置页」）：核心的配置清单（`config.schema`）和最终值（`config.get`）合成一页页、一组组的项；
//! 每一项来自哪一层、写在哪一层、`expect` 写什么、搜索对不对得上。不碰 DOM、不发请求。

/**
 * @typedef {{key: string, type: string, control: string, name: string, description: string, page: string, group: string,
 *   common?: boolean, layers: string[], applies: string, default?: any, options?: {value: any, name: string}[],
 *   min?: number, max?: number, max_chars?: number, element?: string}} SchemaItem 清单里的一项
 * @typedef {{origin: {layer: string, file?: string, line?: number, env?: string}, used: boolean, value: any}} LayerValue
 * @typedef {{value?: any, origin?: LayerValue['origin'], layers?: LayerValue[]}} Entry `config.get` 里一项的最终值和每一层
 * @typedef {{key?: string, level: string, message: string, file?: string, line?: number}} Problem
 * @typedef {SchemaItem & {entry: Entry|null, problems: Problem[], fact?: {value: any}}} Item 合好的一项（`fact`：配置里没写时模型目录给的值，
 *   模型那一页用）
 * @typedef {{id: string, name: string, items: Item[]}} Group
 * @typedef {{id: string, name: string, groups: Group[], problems: Problem[]}} Page
 */

/** 键里有人起的名字的项（`providers.<id>.base_url`）不照通用的行画：交给模型那一页的编辑器。 */
export const isTemplate = (key) => key.includes('<');

/**
 * 合成一页页：照 `pages`、`groups` 的先后，组里常用的排前面；带占位的项不列。项少的页并进别的页（`merge`：页 → 并进哪一页），
 * 个别组挪到别的页（`moveGroups`：组 → 哪一页）；并掉的页不再单列。别的头的项（键名以 `hide` 里的哪一个打头，`tui.`）不列（蓝图第 17 条）。
 * 对不上哪一项的问题挂在 `fallback` 那一页顶上。
 * @param {{pages: {id: string, name: string}[], groups: {id: string, name: string, page: string}[], items: SchemaItem[]}} schema
 * @param {{items: Record<string, Entry>, problems?: Problem[]}} got
 * @param {{fallback?: string, merge?: Record<string, string>, moveGroups?: Record<string, string>, hide?: string[]}} [opts]
 * @returns {Page[]}
 */
export function buildPages(schema, got, opts = {}) {
  const { fallback = 'advanced', merge = {}, moveGroups = {}, hide = [] } = opts;
  const foreign = (key) => hide.some((prefix) => key.startsWith(prefix));
  const known = new Set(schema.items.map((i) => i.key));
  const problems = got.problems ?? [];
  const pages = schema.pages.filter((p) => !merge[p.id]).map((p) => ({ id: p.id, name: p.name, groups: /** @type {Group[]} */ ([]), problems: /** @type {Problem[]} */ ([]) }));
  const byPage = new Map(pages.map((p) => [p.id, p]));
  for (const g of schema.groups) {
    const items = schema.items
      .filter((i) => i.group === g.id && !isTemplate(i.key) && !foreign(i.key))
      .map((i) => ({ ...i, entry: got.items?.[i.key] ?? null, problems: problems.filter((x) => x.key === i.key) }))
      .sort((a, b) => Number(!!b.common) - Number(!!a.common));
    const home = moveGroups[g.id] ?? g.page;
    if (items.length) byPage.get(merge[home] ?? home)?.groups.push({ id: g.id, name: g.name, items });
  }
  // 对不上清单里哪一项的（整份写坏了、不认识的键、只属于带占位那几项的不算）：放那一页顶上。别的头的项连它的问题一起不列，
  // 不认识的键不管叫什么都要提示（改了名的旧键 `tui.startup` 留在文件里，人得看得到去删）
  const loose = problems.filter((x) => !x.key || (!known.has(x.key) && !templateOf(x.key, schema.items)));
  (byPage.get(fallback) ?? pages.at(-1))?.problems.push(...loose);
  return pages.filter((p) => p.groups.length || p.problems.length);
}

/**
 * 名字下面那一行小字（蓝图第 7 条）：只留和「默认、当场生效」不一样的部分；两样都一样的交 `null`（整行不出现）。
 * @param {Entry|null} entry @param {string} applies
 * @returns {{source: ReturnType<typeof sourceOf>|null, applies: string|null}|null}
 */
export function noteOf(entry, applies) {
  const src = sourceOf(entry);
  const source = src.env || !['default', 'none'].includes(src.layer) ? src : null;
  const when = applies === 'now' || applies === 'live' ? null : applies;
  return source || when ? { source, applies: when } : null;
}

/** 一个真键对得上哪一项带占位的（`providers.dev.base_url` → `providers.<id>.base_url`）。 */
export function templateOf(key, items) {
  const parts = splitKey(key);
  return items.find((i) => {
    if (!isTemplate(i.key)) return false;
    const t = i.key.split('.');
    return t.length === parts.length && t.every((seg, n) => seg.startsWith('<') || seg === parts[n]);
  }) ?? null;
}

/** 照 TOML 点号连着的键拆开：带双引号的一段里的点不算（`providers.dev.models."a.b".window`）。 */
export function splitKey(key) {
  const out = [];
  let cur = '';
  let quoted = false;
  for (const ch of key) {
    if (ch === '"') quoted = !quoted;
    else if (ch === '.' && !quoted) {
      out.push(cur);
      cur = '';
    } else cur += ch;
  }
  out.push(cur);
  return out;
}

/**
 * 写在哪一层：能写个人设置的写个人设置，只能写系统配置的写系统配置（M8 只有管理员一个人）。
 * @param {SchemaItem} item
 */
export const layerFor = (item) => (item.layers.includes('personal') ? 'personal' : 'system');

/**
 * `config.set` 的 `expect`：这一层里这一项现在写着什么，`{value}` 或者没写 `{}`。
 * @param {Entry|null} entry
 * @param {string} layer
 */
export function expectFor(entry, layer) {
  const had = entry?.layers?.find((l) => l.origin.layer === layer);
  return had ? { value: had.value } : {};
}

/** 这一层写了这一项没有：写了的能「恢复默认」。 */
export const writtenIn = (entry, layer) => !!entry?.layers?.some((l) => l.origin.layer === layer);

/**
 * 来源那一行小字要的：哪一层、文件、第几行、压着的环境变量。没有最终值的（没默认值、哪一层都没写）是 `none`。
 * @param {Entry|null} entry
 * @returns {{layer: string, file?: string, line?: number, env?: string}}
 */
export function sourceOf(entry) {
  if (!entry?.origin) return { layer: 'none' };
  return { ...entry.origin };
}

/** 值是环境变量引用（`{env: X}`）：交回名字，不是的交 `null`。 */
export const envRef = (value) => (value && typeof value === 'object' && !Array.isArray(value) && typeof value.env === 'string' ? value.env : null);

/**
 * 控件里写的字：`text`、`number` 控件照它显示；人改了以后整串作为 `input` 交给核心，核心照这一项的类型读（时长写 `10m`）。
 * @param {any} value
 */
export function inputText(value) {
  if (value == null) return '';
  if (typeof value === 'string') return value;
  if (typeof value === 'number' || typeof value === 'boolean') return String(value);
  return JSON.stringify(value);
}

/**
 * 搜索：名字、键、说明里有这几个字（不分大小写）。交回对得上的项，带上在哪一页。
 * @param {Page[]} pages
 * @param {string} query
 * @returns {{page: Page, group: Group, item: Item}[]}
 */
export function search(pages, query) {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  const out = [];
  for (const page of pages) {
    for (const group of page.groups) {
      for (const item of group.items) {
        const hay = `${item.name}\n${item.key}\n${item.description}`.toLowerCase();
        if (hay.includes(q)) out.push({ page, group, item });
      }
    }
  }
  return out;
}

/** 哪几页有错误（左栏名字后面一个红点）。 */
export function pagesWithErrors(pages) {
  const bad = (p) => p.problems.some((x) => x.level === 'error') || p.groups.some((g) => g.items.some((i) => i.problems.some((x) => x.level === 'error')));
  return new Set(pages.filter(bad).map((p) => p.id));
}

/** 照 TOML 点号连着的键写一段：只有字母、数字、`-`、`_` 的裸着写，别的带双引号（`"cline-pass/deepseek-v4.1-flash"`）。 */
export const keySegment = (name) => (/^[A-Za-z0-9_-]+$/.test(name) ? name : `"${name.replaceAll('\\', '\\\\').replaceAll('"', '\\"')}"`);

/**
 * 带占位的键换成真键：`providers.<id>.models.<model>.window` + `{id: 'dev', model: 'a/b'}` → `providers.dev.models."a/b".window`。
 * @param {string} template @param {Record<string, string>} names
 */
export function realKey(template, names) {
  return template.split('.').map((seg) => (seg.startsWith('<') ? keySegment(names[seg.slice(1, -1)] ?? seg) : seg)).join('.');
}

/**
 * 模型那一页的一项：照清单里带占位的那一项，换成真键，配上最终值；配置里没写的带上目录给的值（`fact`）。清单里没有这一项的交 `null`。
 * @param {{items: SchemaItem[]}} schema @param {{items: Record<string, Entry>, problems?: Problem[]}} got
 * @param {string} template @param {Record<string, string>} names @param {{value: any}} [fact]
 * @returns {Item|null}
 */
export function itemFor(schema, got, template, names, fact) {
  const spec = schema.items.find((i) => i.key === template);
  if (!spec) return null;
  const key = realKey(template, names);
  return { ...spec, key, entry: got.items?.[key] ?? null, problems: (got.problems ?? []).filter((p) => p.key === key), ...(fact ? { fact } : {}) };
}

/**
 * 清单里一项不带占位的（`models.chat`）配上最终值。
 * @param {{items: SchemaItem[]}} schema @param {{items: Record<string, Entry>, problems?: Problem[]}} got @param {string} key
 * @returns {Item|null}
 */
export function plainItem(schema, got, key) {
  return itemFor(schema, got, key, {});
}

/** 窗口这类大数写短：`1000000` → `1M`，`128000` → `128k`。 */
export function shortCount(n) {
  if (typeof n !== 'number') return '';
  if (n >= 1e6) return `${+(n / 1e6).toFixed(n % 1e6 ? 1 : 0)}M`;
  if (n >= 1e3) return `${+(n / 1e3).toFixed(n % 1e3 ? 1 : 0)}k`;
  return String(n);
}

/** 同一家里重了的显示名（同一个模型的几条线路）：这几个名字下面要写模型名分开它们。 @param {string[]} names */
export function duplicates(names) {
  const seen = new Set();
  const dup = new Set();
  for (const n of names) (seen.has(n) ? dup : seen).add(n);
  return dup;
}
