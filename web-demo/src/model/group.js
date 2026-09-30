// @ts-check
//! 正文的条目分块（蓝图 `web.md`「对话区」）：你的话一条一块；她的条目照回合连成一块，挂在一个头像下面；回报、压缩和清空
//! 那一行（`note`）自己一块，不挂在她的头下（「后台命令、子代理的回报」「压缩、清空」）；可夹在她同一轮中间的（她正在回答时来的
//! 回报），放进她那一块里，头不再画一次（2026-09-30 项目主人定）。插进她正在回答的一轮的话（你的、子代理的），前后照回合分成两块，
//! 后一块接着画、不再写头像和名字（`cont`，同一天项目主人定）。纯函数。

/**
 * @typedef {{key: string, kind: 'user'|'her'|'note', turn?: number|null, item?: any, items: any[], cont?: boolean}} Block
 *   `cont` 的是接着她同一轮的（中间插进来一句话），不再画头像和名字
 */

/** @param {any[]} items @returns {Block[]} */
export function group(items) {
  /** @type {Block[]} */
  const out = [];
  // 编号照回合和这一轮的第几块：在收的回答落了盘换了编号，这一块照旧，头像不重画
  const seen = new Map();
  items.forEach((it, i) => {
    const last = out.at(-1);
    if (it.type === 'user') out.push({ key: it.key, kind: 'user', item: it, items: [] });
    else if (it.type === 'note' && last?.kind === 'her' && inTurn(items, i, last)) last.items.push(it);
    else if (it.type === 'note') out.push({ key: it.key, kind: 'note', item: it, items: [] });
    else if (last?.kind === 'her' && last.turn === it.turn) last.items.push(it);
    else {
      const n = (seen.get(it.turn) ?? 0) + 1;
      seen.set(it.turn, n);
      // 中间插进来一句话、后面还是她这一轮的：接着画，不再画头像和名字（2026-09-30 项目主人定：每插一句头像名字又画一次，不好看）
      const cont = n > 1;
      out.push({ key: `h${it.turn}-${n}`, kind: 'her', turn: it.turn, items: [it], cont });
    }
  });
  return out;
}

/**
 * 第 `i` 条夹没夹在她这一轮中间：后面（跳过别的这种行）接着的是她这一轮的；后面还没有东西的，看她这一轮结束没有（那一块里还没有
 * 收尾那一行的是还在进行，回报刚来、她下一步还没来的那一刻也放进去，不先画成单独一块再挪）。
 */
function inTurn(items, i, block) {
  const next = items.slice(i + 1).find((x) => x.type !== 'note');
  if (next) return next.type !== 'user' && next.turn === block.turn;
  return !block.items.some((x) => x.type === 'done');
}
