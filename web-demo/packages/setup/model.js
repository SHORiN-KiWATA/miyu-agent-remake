// @ts-check
//! 人格、预设、工作区（软件包 `setup`，蓝图 `web.md`「人格、预设、工作区」）：按钮上写什么、实际会用哪个、默认的能不能用、最近用过的目录。
//! 纯函数。

/** `persona.list` 的一项：写错的只有 `persona`、`problem`。 @typedef {{persona: string, name?: string|null, summary?: string|null, layers?: string[], problem?: string}} Persona */
/** `preset.list` 的一项：写错的只有 `preset`、`problem`。 @typedef {{preset: string, name?: string|null, summary?: string|null, layers?: string[], problem?: string}} Preset */

/** 人格写给人看的名字：没有显示名的写编号。 @param {Persona} p */
export const personaName = (p) => p.name || p.persona;

/** 预设写给人看的名字：没有显示名的写编号。 @param {Preset} p */
export const presetName = (p) => p.name || p.preset;

/**
 * 默认的能不能用（第 2 条：没了、文件写错的锁住输入框）：指着的那一项在列表里、没写错。
 * @param {(Persona|Preset)[]} list @param {string|null} id @param {'persona'|'preset'} [key]
 */
export function defaultUsable(list, id, key = 'persona') {
  const hit = id ? list.find((p) => /** @type {any} */ (p)[key] === id) : null;
  return !!hit && !hit.problem;
}

/** 没选预设时用哪个：配置项 `preset.default`，没写的是出厂的 `full`（`protocol.md` 的 `session.create`）。 @param {string|null} chosen @param {string|null} fallback */
export const presetInUse = (chosen, fallback) => chosen ?? fallback ?? 'full';

/**
 * 没选人格时用哪个（`presets.md`「怎么走」第 3 条）：预设写的默认人格，再是配置项 `persona.default`，都没写是出厂的 `engineer`。
 * @param {string|null} chosen @param {string|null} fromPreset @param {string|null} fallback
 */
export const personaInUse = (chosen, fromPreset, fallback) => chosen ?? fromPreset ?? fallback ?? 'engineer';

/** 目录名（按钮上写的）：路径最后一段，`~` 照写。 @param {string} path */
export function dirName(path) {
  const parts = path.replace(/[\\/]+$/, '').split(/[\\/]/);
  return parts.at(-1) || path;
}

/**
 * 输入框里写的路径：只认绝对的（`/…`；Windows 的 `C:\…`、`C:/…`、`\\服务器\…`）和 `~`、`~/…`（相对的不知道照谁接），核心在哪个系统上
 * 都能用；去掉前后空白和末尾的分隔符（根目录本身留着）。不认的是 `null`。
 * @param {string} text
 */
export function readPath(text) {
  const t = text.trim();
  const ok = t === '~' || t.startsWith('~/') || t.startsWith('~\\') || t.startsWith('/') || /^[A-Za-z]:[\\/]/.test(t) || t.startsWith('\\\\');
  if (!ok) return null;
  const cut = t.replace(/[\\/]+$/, '');
  return cut === '' || /^[A-Za-z]:$/.test(cut) ? t.slice(0, cut.length + 1) : cut;
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
  return path.startsWith(`${home}/`) || path.startsWith(`${home}\\`) ? `~${path.slice(home.length)}` : path;
}
