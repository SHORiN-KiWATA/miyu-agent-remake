// @ts-check
//! 待办：演示怎么推进、收成几行（蓝图 `web.md`「待办」，规矩照 `tui.md`「后台命令、子代理和侧边栏」第 4 条，
//! 做法照 TUI 演示的 `jobs/fake.rs`、`ui/sidebar.rs` 的 `todo_lines`；软件包 `todo`）。纯函数。
//!
//! 核心还没有待办：`/demo-todo` 推一份演示的（这个包的设置项 `demo_items`），核心有了以后只换数据源，收成几行照旧。

/** @typedef {'pending'|'active'|'done'} TodoState 没做、在做、做完 */
/** @typedef {{text: string, state: TodoState}} Todo 一项 */
/**
 * @typedef {{kind: 'item', todo: Todo}|{kind: 'folded', count: number}|{kind: 'more', count: number}} Row
 *   一行：一项；做完的收成的一行（`☑ 做完 N 项`）；放不下的收成的最后一行（`… 还有 N 项`）
 */

/** 推一份：第一项在做，别的没做。 */
export function startTodos(texts) {
  return texts.map((text, i) => /** @type {Todo} */ ({ text, state: i === 0 ? 'active' : 'pending' }));
}

/** 推进一项：在做的做完，下一项接着做；都做完了原样交回。交回新的一份，不改原来的。 */
export function advance(todos) {
  const at = todos.findIndex((x) => x.state === 'active');
  if (at < 0) return todos;
  return todos.map((x, i) => {
    if (i === at) return { ...x, state: /** @type {TodoState} */ ('done') };
    if (i === at + 1) return { ...x, state: /** @type {TodoState} */ ('active') };
    return x;
  });
}

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
