// @ts-check
//! 待办：核心的清单换成这里的样子、收成几行（蓝图 `web.md`「待办」，规矩照 `tui.md`「后台命令、子代理和侧边栏」第 4 条，
//! 做法照 TUI 演示的 `ui/sidebar.rs` 的 `todo_lines`；软件包 `todo`）。纯函数。

/** @typedef {'pending'|'active'|'done'} TodoState 没做、在做、做完 */
/** @typedef {{text: string, state: TodoState}} Todo 一项 */
/**
 * @typedef {{kind: 'item', todo: Todo}|{kind: 'folded', count: number}|{kind: 'more', count: number}} Row
 *   一行：一项；做完的收成的一行（`☑ 做完 N 项`）；放不下的收成的最后一行（`… 还有 N 项`）
 */

/** 做完几项、一共几项（头一行的 `待办 2/5`）。 */
export function progress(todos) {
  return { done: todos.filter((x) => x.state === 'done').length, total: todos.length };
}

/** 都做完了（没有待办的不算）。 */
export const allDone = (todos) => todos.length > 0 && todos.every((x) => x.state === 'done');

/**
 * 收成几行。`full` 时全部列出（长的由界面折行）；项数不超过 `rows` 的一项一行；放不下的只露正在做的那几项：
 * 做完的收成一行，接着在做的和后面的，放不下的收成最后一行（`tui.md` 第 4 条）。全做完了（收掉前停着让人看的
 * 那一会儿）留最后一项，前面的收成一行，好看到它打勾。
 * @param {Todo[]} todos
 * @param {number} rows 最多几行（`layout.json` 的 `todo_rows`）
 * @param {boolean} full
 * @returns {Row[]}
 */
export function fold(todos, rows, full) {
  const items = (list) => list.map((todo) => /** @type {Row} */ ({ kind: 'item', todo }));
  if (full || todos.length <= rows) return items(todos);
  if (allDone(todos)) return [{ kind: 'folded', count: todos.length - 1 }, ...items(todos.slice(-1))];
  const open = todos.filter((x) => x.state !== 'done');
  const done = todos.length - open.length;
  /** @type {Row[]} */
  const out = done > 0 ? [{ kind: 'folded', count: done }] : [];
  const room = Math.max(0, rows - out.length);
  const shown = open.length <= room ? open.length : Math.max(0, room - 1);
  out.push(...items(open.slice(0, shown)));
  if (shown < open.length) out.push({ kind: 'more', count: open.length - shown });
  return out;
}

/** 核心的状态 → 这一块的三种（核心 D-3：`pending` 没做、`in_progress` 在做、`completed` 做完；不认识的当没做）。 */
const STATES = { pending: 'pending', in_progress: 'active', completed: 'done' };

/**
 * 核心推来的清单（`subscribe` 回应的 `todos`、瞬时的 `todos.changed`）换成这一块的样子。
 * @param {{content: string, status: string}[]} todos
 * @returns {Todo[]}
 */
export function fromCore(todos) {
  return (todos ?? []).map((x) => ({ text: x.content, state: /** @type {TodoState} */ (STATES[x.status] ?? 'pending') }));
}
