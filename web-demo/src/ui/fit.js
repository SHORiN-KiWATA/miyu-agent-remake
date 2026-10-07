// @ts-check
//! 浮在输入框上面的框（命令列表、会话列表、帮助）放不下时（蓝图 `web.md`「命令列表」第 7 条）：空会话里输入框在窗口中间，上面地方少，
//! 框会顶出窗口上沿、吉祥物（站在开着的浮层顶上）被挤进框里。框里会滚的那一截矮下去，框顶离窗口上沿至少留出 `TOP_ROOM`（吉祥物
//! 站得下），最矮留 `min`；一行一样的列表按整行截（`rows`），不露半行。框里的东西每变一次（筛了、读到了）量一次。

/** 框顶至少离窗口上沿多远：吉祥物站得下 */
export const TOP_ROOM = 72;

/**
 * @param {HTMLElement} box 整个框
 * @param {HTMLElement} list 框里会滚的那一截
 * @param {number} [min] 最矮多高
 * @param {boolean} [rows] 一行一样高的列表：照相邻两行的间隔（行高加行距）按整行截
 */
export function fitAbove(box, list, min = 72, rows = true) {
  list.style.maxHeight = '';
  const over = TOP_ROOM - box.getBoundingClientRect().top;
  if (over <= 0) return;
  const [a, b] = /** @type {HTMLElement[]} */ ([...list.children]);
  const row = !rows || !a ? 0 : b ? b.offsetTop - a.offsetTop : a.offsetHeight;
  const room = Math.max(min, list.clientHeight - over);
  list.style.maxHeight = `${row > 0 ? Math.max(row, Math.floor(room / row) * row) : room}px`;
}
