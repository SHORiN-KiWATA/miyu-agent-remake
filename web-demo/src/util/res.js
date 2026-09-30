// @ts-check
//! 资源：界面上的字、布局的数、人格、主题、时间线和 Markdown 的配置。都是数据，住在 `resources/` 里，代码里不写（`00-设计理念.md` 第六节）。
//! 界面的字、命令的说明按语言写：内核定了界面语言以后照它装（`useTexts`，蓝图 `web.md`「界面语言」）。

import { local, merge } from '../lib/text.js';

/**
 * @typedef {{text: any, layout: any, persona: any, lucide: any, timeline: any, markdown: any, artifacts: any, cards: any,
 *   commands: {commands: import('../model/commands.js').Spec[]}, languages: import('../kernel/language.js').Table,
 *   language: import('../kernel/language.js').Language, human: {tools: Record<string, any>, said: Record<string, string>}}} Res
 *   `human` 是桥给的给人看的字（`web.human`）；`language` 是定下来的界面语言，`text`、`commands` 的说明照它装
 */

/** 读进来的全部资源。页面起来时 `loadResources` 装满；测试里直接往里放。 */
export const res = /** @type {Res} */ (/** @type {any} */ ({}));

const FILES = { layout: 'layout.json', persona: 'persona.json', lucide: 'lucide.json', timeline: 'timeline.json', markdown: 'markdown.json',
  artifacts: 'artifacts.json', cards: 'cards.json', commands: 'commands.json', languages: 'languages.json' };

/** 命令清单的原样（说明每种语言各一句）：换了语言照它重新挑 */
let commands = /** @type {any[]} */ ([]);

/**
 * 读 `resources/` 下的一个 JSON。
 *
 * # Errors
 * 读不到、不是 JSON，照原因抛出来。
 */
async function get(base, p) {
  const r = await fetch(new URL(p, base));
  if (!r.ok) throw new Error(`读不了资源 ${p}：${r.status}`);
  return r.json();
}

/**
 * 读不按语言分的资源（界面的字另外装，见 `useTexts`）。`base` 是 `resources/` 的地址。
 *
 * # Errors
 * 哪个文件读不到、不是 JSON，照原因抛出来：页面起不来，由入口写在页面上。
 */
export async function loadResources(base) {
  const loaded = await Promise.all(Object.entries(FILES).map(async ([k, p]) => [k, await get(base, p)]));
  Object.assign(res, Object.fromEntries(loaded));
  commands = res.commands.commands;
  return res;
}

/**
 * 照界面语言装字：退回的那一份打底，这一种的盖上去（缺的一句退回）；命令的说明挑这一种。
 * @param {URL} base `resources/` 的地址
 * @param {import('../kernel/language.js').Language} lang
 *
 * # Errors
 * 字的文件读不到、不是 JSON，照原因抛出来。
 */
export async function useTexts(base, lang) {
  const [fallback, own, summary] = await Promise.all([
    get(base, `text/${lang.fallback}.json`),
    lang.code === lang.fallback ? null : get(base, `text/${lang.code}.json`),
    // 时间线收起那一行用别的一种（跟着浏览器时的英文）：再读那一份
    lang.summary === lang.code ? null : get(base, `text/${lang.summary}.json`),
  ]);
  settle(fallback, own, lang, commands, summary);
}

/**
 * 装好的字放进 `res`（`useTexts` 读完调；测试直接给两份字）。
 * @param {any} fallback 退回的那一份
 * @param {any} own 这一种的（和退回的是同一种时 `null`）
 * @param {import('../kernel/language.js').Language} lang
 * @param {any[]} list 命令清单的原样（说明每种语言各一句）
 * @param {any} [summary] 时间线收起那一行用别的一种时，那一种的字（只取 `timeline.summary`）
 */
export function settle(fallback, own, lang, list, summary = null) {
  const text = own ? merge(fallback, own) : fallback;
  res.text = summary ? merge(text, { timeline: { summary: summary.timeline.summary } }) : text;
  res.language = lang;
  res.commands = { commands: list.map((c) => ({ ...c, summary: local(c.summary, lang) })) };
}

/** 把 `{名字}` 换成字段的值；没给的留着原样，一眼看得出漏了哪个。 */
export function fill(template, fields = {}) {
  return template.replace(/\{(\w+)\}/g, (all, k) => (fields[k] == null ? all : String(fields[k])));
}

/** 界面上的一句字：`t('status.online')`，带字段的换进去。找不到的回路径本身，一眼看得出漏了哪句。 */
export function t(path, fields) {
  const v = path.split('.').reduce((o, k) => (o == null ? o : o[k]), res.text);
  if (v == null) return path;
  return typeof v === 'string' ? fill(v, fields) : v;
}
