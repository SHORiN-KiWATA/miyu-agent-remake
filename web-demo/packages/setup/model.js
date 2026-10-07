// @ts-check
//! 人格、工作区（软件包 `setup`，蓝图 `web.md`「人格、预设、工作区」）：按钮上写什么、默认的人格能不能用、最近用过的目录。纯函数。

/** `persona.list` 的一项：写错的只有 `persona`、`problem`。 @typedef {{persona: string, name?: string|null, summary?: string|null, layers?: string[], problem?: string}} Persona */

/** 人格写给人看的名字：没有显示名的写编号。 @param {Persona} p */
export const personaName = (p) => p.name || p.persona;

/**
 * 默认的人格能不能用（第 2 条：没了、文件写错的锁住输入框）：配置项 `persona.default` 指着的那一项在列表里、没写错。
 * @param {Persona[]} list @param {string|null} def
 */
export function defaultUsable(list, def) {
  const hit = def ? list.find((p) => p.persona === def) : null;
  return !!hit && !hit.problem;
}

/** 目录名（按钮上写的）：路径最后一段，`~` 照写。 @param {string} path */
export function dirName(path) {
  const parts = path.replace(/\/+$/, '').split('/');
  return parts.at(-1) || path;
}

/** 输入框里写的路径：只认 `~`、`~/…`、`/…`（相对的不知道照谁接）；去掉前后空白和末尾的 `/`。不认的是 `null`。 @param {string} text */
export function readPath(text) {
  const t = text.trim();
  if (!(t === '~' || t.startsWith('~/') || t.startsWith('/'))) return null;
  return t.length > 1 ? t.replace(/\/+$/, '') : t;
}

/** 记一个最近用过的目录：排到最前，重复的去掉，最多 `max` 个；默认工作区不记。 @param {string[]} list @param {string} path @param {string} fallback @param {number} max */
export function remember(list, path, fallback, max) {
  if (path === fallback) return list;
  return [path, ...list.filter((x) => x !== path)].slice(0, max);
}

/** 家目录下的写成 `~/…`（显示用）。 @param {string} path @param {string|null} home */
export function tilde(path, home) {
  if (!home) return path;
  if (path === home) return '~';
  return path.startsWith(`${home}/`) ? `~${path.slice(home.length)}` : path;
}
