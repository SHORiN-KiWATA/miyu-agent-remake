// @ts-check
//! 后台任务浮层怎么分段（软件包 `jobs`，蓝图 `web.md`「后台任务」第 3 条）：「进行中」是在跑的、停在半路的，子代理下面在跑的、
//! 停在半路的缩进一层接在它下面（哪一层的都算，父子代理结束了的也接出来）；「已结束」是结束的。嵌套里结束的不单列，算进它那一层
//! 那一行的「派了 N 个」（`spawned`，它直接派的有几个）。「进行中」每一行还带着文件树的线怎么画（和左栏一样）：是不是这一层最后
//! 一个（`last`），上面几层要不要往下画竖线（`guides`，第 k 个是第 k+2 层），下面有没有接着它的（`kids`）；父子代理已经结束、
//! 不在「进行中」的（`orphan`），上面没有能接的，不画线。纯函数。

/**
 * @typedef {import('../../src/lib/jobs.js').Task & {owner: string, kids: Node[]}} Node
 * @typedef {{task: Node, depth: number, spawned: number, last?: boolean, guides?: boolean[], kids?: boolean, orphan?: boolean}} Line
 *   一行：哪个任务、缩进几层、它派了几个；「进行中」的带着树线
 */

const live = (/** @type {Node} */ x) => x.state === 'running' || x.state === 'paused';

/**
 * @param {Node[]} tree 这个会话的任务，连嵌套的
 * @returns {{live: Line[], done: Line[], counts: {live: number, done: number}}}
 */
export function sections(tree) {
  /** @type {Line[]} */
  const going = [];
  /** @type {Line[]} */
  const done = [];
  /**
   * 一个在跑的，接着它下面在跑的（一路往下）。
   * @param {Node} x
   * @param {number} depth
   * @param {{last: boolean, guides: boolean[], orphan: boolean}} line
   */
  const addLive = (x, depth, line) => {
    const kids = x.kids.filter(live);
    going.push({ task: x, depth, spawned: x.kids.length, ...line, kids: kids.length > 0 });
    // 第一层下面的竖线接在第一层的记号上，不用接更上面的；再往下的接上它上一层的
    const guides = depth === 0 ? [] : [...line.guides, !line.last];
    kids.forEach((k, j) => addLive(k, depth + 1, { last: j === kids.length - 1, guides, orphan: false }));
  };
  for (const x of tree) {
    if (live(x)) addLive(x, 0, { last: true, guides: [], orphan: false });
    else {
      done.push({ task: x, depth: 0, spawned: x.kids.length });
      const kids = x.kids.filter(live);
      kids.forEach((k, j) => addLive(k, 1, { last: j === kids.length - 1, guides: [], orphan: true }));
    }
  }
  return { live: going, done, counts: { live: going.length, done: done.length } };
}
