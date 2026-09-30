// @ts-check
//! 字的小工具（`lib/`：纯的，谁都能用）：模板里的字段、照路径取一句、按语言挑一块、缺的字一层层退回（蓝图 `web.md`「界面语言」）。

/** 把 `{名字}` 换成字段的值；没给的留着原样，一眼看得出漏了哪个。 */
export function fill(template, fields = {}) {
  return template.replace(/\{(\w+)\}/g, (all, k) => (fields[k] == null ? all : String(fields[k])));
}

/**
 * 照路径（`status.online`）取一句，带字段的换进去。找不到的回路径本身，一眼看得出漏了哪句；不是一句字（一组）的原样交回。
 * @param {any} table 字的表
 * @param {string} path
 * @param {Record<string, unknown>} [fields]
 */
export function lookup(table, path, fields) {
  const v = path.split('.').reduce((o, k) => (o == null ? o : o[k]), table);
  if (v == null) return path;
  return typeof v === 'string' ? fill(v, fields) : v;
}

/** @typedef {{code: string, fallback: string}} Lang 界面语言：用哪一种、缺的退回哪一种 */

/**
 * 按语言写的一块（`{zh: …, ja: …}`）挑这一种，没有的退回；不是按语言写的（一串、一句字、键里没有这两种语言的对象）原样交回。
 * @param {any} value
 * @param {Lang} lang
 */
export function local(value, lang) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return value;
  if (lang.code in value) return value[lang.code];
  return lang.fallback in value ? value[lang.fallback] : value;
}

/**
 * 两份字合成一份：上面有的用上面的，没有的用下面的（缺的字退回）；对象一层层合，一句字、一组（数组）整个换。
 * @param {any} base 下面的（退回的那一种）
 * @param {any} over 上面的（这一种）
 */
export function merge(base, over) {
  if (over === undefined) return base;
  const plain = (v) => v && typeof v === 'object' && !Array.isArray(v);
  if (!plain(base) || !plain(over)) return over;
  const out = { ...base };
  for (const [k, v] of Object.entries(over)) out[k] = merge(base[k], v);
  return out;
}
