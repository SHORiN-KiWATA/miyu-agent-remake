// @ts-check
//! 后台命令的预览（软件包 `jobs`，蓝图 `web.md`「后台任务」第 3 条最后一点）：浮层里点一条后台命令，在它下面展开一块：「命令」
//! 一段（命令本身），「输出」一段（最后 `preview_lines` 行，等宽，放不下在里面滚）。输出照核心的 `job.output` 读（施工 7-4 补）：
//! 打开时读一次，跑着的每秒再读（由浮层的计时推 `refresh`）；输出贴着底，人往上翻了不拽回去。连的核心太旧、没有这个接口的写一句。

import { h } from '../../src/lib/dom.js';

/** JSON-RPC 的「没有这个方法」：连的核心比施工 7-4 补旧，没有 `job.output` */
const NO_METHOD = -32601;

export class Preview {
  /**
   * @param {import('./sections.js').Node} task
   * @param {(path: string, fields?: any) => string} t 这个包的字
   * @param {(owner: string, job: string) => Promise<{output: string, lines?: number, truncated?: boolean, running?: boolean}>} read
   */
  constructor(task, t, read) {
    this.task = task;
    this.t = t;
    this.read = read;
    this.out = h('pre.jobs-output', t('preview.loading'));
    this.head = h('div.jobs-preview-note', { hidden: true });
    const section = (label, ...body) => h('div.jobs-preview-part', h('div.jobs-preview-label', label), ...body);
    this.el = h('div.jobs-preview',
      section(t('preview.command'), h('pre.jobs-command', task.command ?? t('preview.no_command'))),
      section(t('preview.output'), this.head, this.out));
    /** 在读（上一次还没回来的不再发） */
    this.busy = false;
    /** 读完了、不用再读（结束了的读一次就够；接口没有、任务没了的也不再读） */
    this.done = false;
  }

  /** 读一次（上一次没回来、不用再读的不读）。 */
  async refresh() {
    if (this.busy || this.done) return;
    this.busy = true;
    const t = this.t;
    try {
      const got = await this.read(this.task.owner, this.task.job);
      const pre = this.out;
      const stick = pre.scrollTop + pre.clientHeight >= pre.scrollHeight - 4;
      pre.textContent = got.output || t('preview.empty');
      this.head.hidden = !got.truncated;
      const shown = got.output ? got.output.replace(/\n$/, '').split('\n').length : 0;
      this.head.textContent = got.truncated ? t('preview.tail', { total: got.lines ?? '?', shown }) : '';
      if (stick) pre.scrollTop = pre.scrollHeight;
      if (!got.running) this.done = true;
    } catch (err) {
      this.done = true;
      const e = /** @type {any} */ (err);
      const known = e?.reason ? t(`reasons.${e.reason}`) : null;
      this.out.textContent = e?.code === NO_METHOD ? t('preview.no_api')
        : known && known !== `reasons.${e.reason}` ? known : e?.message ?? String(err);
    } finally {
      this.busy = false;
    }
  }
}
