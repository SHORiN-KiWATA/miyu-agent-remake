// @ts-check
//! 左栏的树（蓝图 `web.md`「左栏」的「子代理的会话」「会话表」）：子代理挂在派它的会话下面，一层层往下。一层里：在跑的、停在半路的
//! 一个一行，结束的收成一行「已完成 N 个」（点开了列出来）。父会话默认：有子代理在跑的、看着的在它下面的展开，别的收着（看着父会话、
//! 都跑完了的也收）；子代理下面的（孙代理）默认收着，子代理带着下面在跑的有几个（`kidCount`，标题前面写 `(N)`；一层层往下都算，
//! 结束的、停在半路的不算），顶层的收着时也带；
//! 看着的在里面的展开；
//! 人点过的照人点的。收着的里面有在跑的记着（`busy`，那一行转圈）。每一行带着文件树的线要怎么画：是不是这一层最后一个（`last`），
//! 上面几层还要不要往下画竖线（`guides`）。纯函数，交回一行一行，左栏照它画。

/**
 * @typedef {{session: string, title: string|null, running: boolean, paused?: boolean}} Node 一个会话（顶层的还带着左栏一项的别的格）
 * @typedef {{kind: 'session', session: string, depth: number, item: any, hasKids: boolean, open: boolean, kidCount: number, busy: boolean, last: boolean, guides: boolean[]}
 *   |{kind: 'done', parent: string, depth: number, count: number, open: boolean, last: boolean, guides: boolean[]}} Row
 *   一行：一个会话；「已完成 N 个」
 */

/**
 * @param {any[]} tops 顶层的会话（左栏一项）
 * @param {(id: string) => Node[]} kids 一个会话下面的子代理，最新的在前
 * @param {{current: string|null, open: Map<string, boolean>, done: Set<string>}} state 看着哪个、人点过的开关、点开了「已完成」的
 * @returns {Row[]}
 */
export function treeRows(tops, kids, state) {
  /** 这个会话下面（一路往下）有没有合 `pred` 的，同一个不找两遍 */
  const holds = (id, pred, seen = new Set()) => {
    if (seen.has(id)) return false;
    seen.add(id);
    return kids(id).some((k) => pred(k) || holds(k.session, pred, seen));
  };
  /** 这个会话下面（一层层往下）在跑的子代理一共几个，同一个不数两遍 */
  const runningUnder = (id, seen = new Set([id])) => kids(id).reduce((n, k) => {
    if (seen.has(k.session)) return n;
    seen.add(k.session);
    return n + (k.running ? 1 : 0) + runningUnder(k.session, seen);
  }, 0);
  const hasCurrent = (id) => holds(id, (k) => k.session === state.current);
  const hasRunning = (id) => holds(id, (k) => k.running);
  const live = (k) => k.running || k.paused;
  /** @type {Row[]} */
  const rows = [];
  /**
   * @param {any} node
   * @param {number} depth
   * @param {Set<string>} seen 上面几层（防着绕回来）
   * @param {boolean} last 是不是这一层最后一个
   * @param {boolean[]} guides 上面几层要不要往下画竖线
   */
  const walk = (node, depth, seen, last, guides) => {
    const id = node.session;
    const children = kids(id).filter((k) => !seen.has(k.session));
    // 顶层的父会话：有在跑的、看着的在下面的展开；子代理下面的（孙代理）只在看着的在下面时展开
    const fallback = depth === 0 ? hasCurrent(id) || hasRunning(id) : hasCurrent(id);
    const open = children.length > 0 && (state.open.get(id) ?? fallback);
    rows.push({ kind: 'session', session: id, depth, item: node, hasKids: children.length > 0, open, kidCount: depth > 0 || !open ? runningUnder(id) : 0, busy: !open && hasRunning(id), last, guides });
    if (!open) return;
    const next = new Set([...seen, id]);
    const going = children.filter(live);
    const done = children.filter((k) => !live(k));
    // 子代理这一层的竖线：顶层下面的一层不用接上面的（顶层没有线），再往下的接上它上一层的
    const below = depth === 0 ? [] : [...guides, !last];
    going.forEach((k, j) => walk(k, depth + 1, next, j === going.length - 1 && !done.length, below));
    if (!done.length) return;
    const doneOpen = state.done.has(id) || done.some((k) => k.session === state.current || hasCurrent(k.session));
    rows.push({ kind: 'done', parent: id, depth: depth + 1, count: done.length, open: doneOpen, last: !doneOpen, guides: below });
    if (doneOpen) done.forEach((k, j) => walk(k, depth + 1, next, j === done.length - 1, below));
  };
  tops.forEach((top) => walk(top, 0, new Set(), true, []));
  return rows;
}

/**
 * 顶层最多露几个（蓝图「会话表」）：前 `max` 个；正在看的不在里面的排在最后照样露。`more` 是还有没露的（下面写「查看全部」）。
 * @template {{session: string}} T
 * @param {T[]} tops
 * @param {string|null} current
 * @param {number} max
 */
export function capTops(tops, current, max) {
  const shown = tops.slice(0, max);
  const extra = current && !shown.some((x) => x.session === current) ? tops.find((x) => x.session === current) : null;
  return { shown: extra ? [...shown, extra] : shown, more: tops.length > max };
}

/**
 * 从主会话一层层到这个会话（蓝图「对话区」的「子代理的会话顶上那条路径」）：照派它的会话往上找，绕回来的不走两遍。
 * @param {string} id
 * @param {(id: string) => string|null} parentOf
 */
export function pathOf(id, parentOf) {
  const path = [id];
  const seen = new Set(path);
  for (let up = parentOf(id); up && !seen.has(up); up = parentOf(up)) {
    seen.add(up);
    path.unshift(up);
  }
  return path;
}
