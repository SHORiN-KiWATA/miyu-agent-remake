// @ts-check
//! 时间线的段和步（蓝图 `web.md`「时间线的数」，照 TUI 演示的 `transcript/steps.rs`）：她开口之前的一串步骤，思考、调工具。
//! 段和步照核心的条目拼（`model/entries.js` 的 `segmentOf`、`stepOf`，核心 9-8）；这里是它们的形状和画的时候要问的。

/**
 * @typedef {{key: string, kind: 'thought', text: string, state: 'thinking'|'done', start: number|null, end: number|null}} Thought
 * @typedef {{key: string, kind: 'tool', name: string, args: string, parsed: any, state: 'preparing'|'running'|'done',
 *   status: string|null, output: string, said: {key: string, fields: Record<string, string>}|null, callId: string|null,
 *   start: number|null, end: number|null, duration: number|null, job?: string|null, toTitle?: string|null,
 *   title?: {name: string, object?: string, said?: string}|null, lines?: {added: number, removed: number}|null, images?: any[], approval?: any}} Tool
 *   `duration` 是这一步的用时（执行命令写在名字后面）；`job` 是派出去的任务的编号（派子代理那一行写它）；`toTitle` 是留言发给的那个子代理的
 *   标题（点开写在「发给」里；找不到的是 `null`）；`title`、`lines`、`images`、`approval` 是核心给的标题那一句、改了多少行、结果里的图、确认
 * @typedef {Thought|Tool} Step
 * @typedef {{type: 'steps', key: string, turn: number|null, finished: boolean, steps: Step[], summary?: {failed: boolean, spans: {text: string, tone: string}[]}}} Segment
 *   `summary` 是收起那一行（核心给的）
 */

/**
 * 在转圈的那一步（`tui.md`「时间线」第 19 条）：她还在写的（思考中、准备……）是最后那一步；都写完了，是最前面那个
 * 没结果的（核心不报哪一个开始跑了，排在前面的就是在跑的那个）。都有结果了是 `null`。
 * @param {Segment} segment
 */
export function active(segment) {
  const last = segment.steps.at(-1);
  if (last && (last.state === 'thinking' || last.state === 'preparing')) return segment.steps.length - 1;
  const i = segment.steps.findIndex((s) => s.state !== 'done');
  return i < 0 ? null : i;
}
