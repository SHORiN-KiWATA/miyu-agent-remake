// @ts-check
//! 人格、预设、工作区（软件包 `setup`，蓝图 `web.md`「人格、预设、工作区」）：按钮上写什么、实际会用哪个、默认的能不能用、最近用过的目录。
//! 纯函数。

/** `persona.list` 的一项：写错的只有 `persona`、`problem`。 @typedef {{persona: string, name?: string|null, summary?: string|null, problem?: string}} Persona */
/** `preset.list` 的一项：写错的只有 `preset`、`problem`。 @typedef {{preset: string, name?: string|null, summary?: string|null, problem?: string}} Preset */

/** 人格写给人看的名字：没有显示名的写编号。 @param {Persona} p */
export const personaName = (p) => p.name || p.persona;

/** 预设写给人看的名字：没有显示名的写编号。 @param {Preset} p */
export const presetName = (p) => p.name || p.preset;

/**
 * `check` 的问题里属于这个人格、预设的（照文件在 `personas/<编号>/`、`presets/<编号>.toml` 下认，哪一层都算）。
 * `persona.list` 的 `problem` 是给日志的英文原话，人看的那一句（照连接的语言）在 `check` 里（项目主人 2026-10-08 定：写中文）。
 * @param {{kind: string, file?: string, line?: number, message: string}[]} problems @param {'persona'|'preset'} kind @param {string} id
 * 文件只留人格目录里的那一截（`persona.toml`、`prompts/examples.md`）、预设的文件名：在哪一层、数据根在哪是核心怎么存，不往界面上露
 * （项目主人 2026-10-08）；手改文件的人知道自己改的是哪个人格。
 * @returns {{line: number|null, message: string, file: string}[]}
 */
export function problemsOf(problems, kind, id) {
  const tail = kind === 'persona' ? `personas/${id}/` : `presets/${id}.toml`;
  return problems
    .filter((p) => p.kind === kind && typeof p.file === 'string')
    .map((p) => ({ p, f: /** @type {string} */ (p.file).replace(/\\/g, '/') }))
    .filter(({ f }) => f.startsWith(tail) || f.includes(`/${tail}`))
    .map(({ p, f }) => ({
      line: Number.isInteger(p.line) ? /** @type {number} */ (p.line) : null,
      message: p.message,
      file: kind === 'persona' ? f.slice(f.lastIndexOf(tail) + tail.length) : `${id}.toml`,
    }));
}

/**
 * 默认的能不能用（第 2 条：预设没了、文件写错的锁住输入框；人格只看写错的，没了的当没设）：指着的那一项在列表里、没写错。
 * @param {(Persona|Preset)[]} list @param {string|null} id @param {'persona'|'preset'} [key]
 */
export function defaultUsable(list, id, key = 'persona') {
  const hit = id ? list.find((p) => /** @type {any} */ (p)[key] === id) : null;
  return !!hit && !hit.problem;
}

/** 没选预设时用哪个：配置项 `preset.default`，没写的是出厂的 `full`（`protocol.md` 的 `session.create`）。 @param {string|null} chosen @param {string|null} fallback */
export const presetInUse = (chosen, fallback) => chosen ?? fallback ?? 'full';

/**
 * 实际用哪个人格：选了的；明着不用人格（`false`）的是没有；没选的照配置项 `persona.default`，指着没有的当没设（核心 2026-10-08）；
 * 都没有是没有人格（出厂不带人格）。预设不再带默认人格（2026-10-08 项目主人：人格、预设互不引用）。
 * @param {string|false|null} chosen @param {string|null} fallback @param {Persona[]} list
 * @returns {string|null}
 */
export function personaInUse(chosen, fallback, list) {
  if (chosen === false) return null;
  if (chosen) return chosen;
  return fallback && list.some((p) => p.persona === fallback) ? fallback : null;
}

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

/** 示范对话的一对：你说的、人格回的。 @typedef {{user: string, assistant: string}} Pair */
/** `persona.set`、`preset.set` 的一项改动。 @typedef {{key: string, value?: string, unset?: true}} Change */

/**
 * 说明改了发什么：写了的发字，清空了发空字（就是没有说明，盖住出厂的那句；核心 P-3 再补）；没变的什么都不发。
 * 2026-10-08 项目主人：说明要能清空（原来发 `unset`，出厂的回到出厂那句，看着清不掉）。
 * @param {'persona'|'preset'} kind @param {string} before @param {string} after @returns {Change[]}
 */
export function summaryChange(kind, before, after) {
  const text = after.trim();
  if (text === before.trim()) return [];
  return [{ key: `${kind}.summary`, value: text }];
}

/**
 * 编辑器里的人格（详情的四样）和读进来时提示词的版本（存的时候当 `expect` 带回去，防覆盖别处的改动）。
 * @typedef {{name: string, summary: string, persona: string, reminders: string, pairs: Pair[], versions: {persona: string|null, reminders: string|null, examples: string|null}}} PersonaDraft
 */

/** 示范对话要存的那几对：字前后的空白去掉，两头都空的不要。 @param {Pair[]} pairs @returns {Pair[]} */
export function cleanPairs(pairs) {
  return pairs.map((p) => ({ user: p.user.trim(), assistant: p.assistant.trim() })).filter((p) => p.user || p.assistant);
}

/** 只写了一头的那一对是第几对（从 0 起，照编辑器里的先后）；都齐的是 -1。 @param {Pair[]} pairs */
export function halfPair(pairs) {
  return pairs.findIndex((p) => !p.user.trim() !== !p.assistant.trim());
}

/**
 * 点「保存」发什么（`persona.set` 的 `changes`、`prompts`）：和读进来的比，变了的才发；提示词带读进来时的版本。
 * 人设、角色扮演提示清空了发空字（人要的是「不要这一段」，`unset` 在出厂人格上会回到出厂的字）；示范对话没有了发空的一对对。
 * 说明清空了发空字（没有说明）。
 * @param {PersonaDraft} before @param {PersonaDraft} after
 * @returns {{changes?: Change[], prompts?: Record<string, any>}|null} 什么都没变是 `null`
 */
export function personaSave(before, after) {
  /** @type {{changes?: Change[], prompts?: Record<string, any>}} */
  const out = {};
  /** @type {Change[]} */
  const changes = [];
  const name = after.name.trim();
  if (name && name !== before.name) changes.push({ key: 'persona.name', value: name });
  changes.push(...summaryChange('persona', before.summary, after.summary));
  if (changes.length) out.changes = changes;
  const prompts = {};
  for (const key of /** @type {const} */ (['persona', 'reminders'])) {
    if (after[key].trim() !== before[key].trim()) prompts[key] = { text: after[key].trim(), expect: before.versions[key] };
  }
  const pairs = cleanPairs(after.pairs);
  if (JSON.stringify(pairs) !== JSON.stringify(cleanPairs(before.pairs))) prompts.examples = { pairs, expect: before.versions.examples };
  if (Object.keys(prompts).length) out.prompts = prompts;
  return out.changes || out.prompts ? out : null;
}
