// @ts-check
//! 跳转条上正在看哪一句（蓝图 `web.md`「右边的跳转条」第 1 条）。纯函数。

/**
 * 右边的跳转条上正在看哪一句（蓝图「右边的跳转条」第 1 条）：顶边在视口上面 `at`（几分之几）以内的最后一句；一句都没到
 * 的算第一句；一句都没有的是 `-1`。
 * @param {number[]} tops 每一句的顶边离视口顶边多远（px，照先后，往上滚出去的是负的）
 * @param {number} view 视口多高
 * @param {number} at 设置项 `current_at`
 */
export function reading(tops, view, at) {
  if (!tops.length) return -1;
  const line = view * at;
  let found = 0;
  tops.forEach((top, i) => {
    if (top <= line) found = i;
  });
  return found;
}
