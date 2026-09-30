// @ts-check
//! 资源：界面上的字、布局的数、人格、主题、时间线和 Markdown 的配置。都是数据，住在 `resources/` 里，代码里不写（`00-设计理念.md` 第六节）。

/**
 * @typedef {{text: any, layout: any, persona: any, lucide: any, timeline: any, markdown: any, artifacts: any, cards: any,
 *   commands: {commands: import('../model/commands.js').Spec[]},
 *   human: {tools: Record<string, any>, said: Record<string, string>}}} Res
 *   `human` 是桥给的给人看的字（`web.human`）
 */

/** 读进来的全部资源。页面起来时 `loadResources` 装满；测试里直接往里放。 */
export const res = /** @type {Res} */ (/** @type {any} */ ({}));

const FILES = { text: 'text/zh.json', layout: 'layout.json', persona: 'persona.json', lucide: 'lucide.json', timeline: 'timeline.json', markdown: 'markdown.json',
  artifacts: 'artifacts.json', cards: 'cards.json', commands: 'commands.json' };

/**
 * 读全部资源。`base` 是 `resources/` 的地址。
 *
 * # Errors
 * 哪个文件读不到、不是 JSON，照原因抛出来：页面起不来，由入口写在页面上。
 */
export async function loadResources(base) {
  const get = async (p) => {
    const r = await fetch(new URL(p, base));
    if (!r.ok) throw new Error(`读不了资源 ${p}：${r.status}`);
    return r.json();
  };
  const loaded = await Promise.all(Object.entries(FILES).map(async ([k, p]) => [k, await get(p)]));
  Object.assign(res, Object.fromEntries(loaded));
  return res;
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
