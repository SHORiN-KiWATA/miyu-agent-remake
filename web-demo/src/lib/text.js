// @ts-check
//! 字的小工具（`lib/`：纯的，谁都能用）：模板里的字段、照路径取一句。

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
