// @ts-check
//! 运行状态行（软件包 `pulse`，蓝图 `web.md`「运行状态行」「排队的消息」）：挂进输入框上面的挂载位 `composer.above`（排在
//! 待办后面）；照对话区每画一次发的事件 `view.changed` 画：在跑的那一轮、这一轮出过的事（换词）、在等的重试、排着的话；在等你确认、
//! 回答时（会话状态的 `waiting`）换成静止的「等待回答」「等待批准」。
//! 提示浮在它上面：写一个页面变量 `--pulse-lines`（它占几行）。停用了回答时那一行没有，排着的话照样发。

import { PulseLine } from './line.js';
import { entriesBeat, statusRetry, localWords } from './model.js';

/** @param {any} ctx */
export function apply(ctx) {
  // 词库照界面语言挑（每一档每种语言各一组）
  const line = new PulseLine({ ...ctx.config, words: localWords(ctx.config.words, ctx.local) }, (path, fields) => ctx.text(path, fields));
  const root = document.documentElement;
  ctx.effect(() => () => {
    line.set(null);
    root.style.removeProperty('--pulse-lines');
  });
  ctx.slots.mount('composer.above', { id: 'pulse', order: 20, render: () => line.el });
  const draw = (v) => {
    const run = v.running;
    // 在等什么、重试、出过的事都照会话状态和条目（核心 9-8）
    const st = v.status;
    const kind = st?.waiting?.[0]?.what ?? null;
    const wait = kind ? ctx.text(kind === 'approve' ? 'waiting_approve' : 'waiting_ask') : null;
    line.set(run ? {
      id: `${v.session}:${run.turn}`,
      start: run.start,
      beat: entriesBeat(v.entries, st),
      retry: statusRetry(st, run.turn),
      queued: v.queued.map((q) => q.text),
      waiting: wait,
    } : null);
    root.style.setProperty('--pulse-lines', String(1 + (run ? v.queued.length : 0)));
  };
  ctx.on('view.changed', draw);
}
