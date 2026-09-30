// @ts-check
//! 左栏多选（蓝图 `web.md`「左栏」的「批量删除」）：点一项勾上、再点去掉；`Shift` 点把上一次点的和这一次之间的都勾上。纯函数，
//! 交回新的一份，不改原来的。

/** 点一项：没勾的勾上，勾了的去掉。 */
export function toggle(selected, id) {
  const next = new Set(selected);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  return next;
}

/**
 * `Shift` 点：上一次点的（`from`）和这一次（`to`）之间，含两头，照会话表的先后都勾上；上一次点的不在表里的只勾这一项。
 * @param {Set<string>} selected
 * @param {string[]} order 会话表的先后
 * @param {string|null} from
 * @param {string} to
 */
export function range(selected, order, from, to) {
  const next = new Set(selected);
  const a = from == null ? -1 : order.indexOf(from);
  const b = order.indexOf(to);
  if (a < 0 || b < 0) {
    next.add(to);
    return next;
  }
  for (const id of order.slice(Math.min(a, b), Math.max(a, b) + 1)) next.add(id);
  return next;
}
