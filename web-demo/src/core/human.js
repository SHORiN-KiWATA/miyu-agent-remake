// @ts-check
//! 给人看的字（蓝图 `web.md`「时间线的数」第 5 条）：工具的显示名、对象是哪个参数、结果那一句的模板，和终端界面、`miyu ask`
//! 同一份。握手以后照界面语言问核心的 `human.get`（核心施工 W-1；原来由桥读资源目录经 `web.human` 给）。

/**
 * @typedef {{tools: Record<string, {name: string, subject?: string, icon?: string, block?: string}>, said: Record<string, string>}} Human
 *   `said` 的编号带位置（`core/…`、`software/<包>/…`），模板是原文，字段由页面换（`model/words.js` 的 `say`）
 */

/**
 * 问核心要这一种语言的字：只留页面用得上的 `tools`、`said`，缺的格子当空的。问不到的照原样报出去，由开机那里决定怎么办。
 * @param {import('./connection.js').Connection} conn
 * @param {string} language 界面语言的代码（`zh`、`ja`）；核心那边哪个包没有这种语言的照英文
 * @returns {Promise<Human>}
 */
export async function loadHuman(conn, language) {
  const got = await conn.request('human.get', { language });
  return { tools: got?.tools ?? {}, said: got?.said ?? {} };
}
