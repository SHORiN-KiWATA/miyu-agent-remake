// @ts-check
//! 了结以后留下的（蓝图 `web.md`「确认和提问」第 6 条）怎么画（`node`）：真的由正文那一层照日志排、经挂载位 `chat.item` 交给这里画
//! （`index.js`），夹在她这一轮里；演示的只在这一页里（`addLocal`：钉在答的那一刻正文的末尾，`anchor`）。留什么由 `model.js` 的
//! `report`、`reportOf` 算，这里只画。

import { h, icon } from '../../src/lib/dom.js';

export class Reports {
  /**
   * @param {(key: string, fields?: Record<string, any>) => string} text
   * @param {{anchor: (node: HTMLElement) => string, place: (where: string, node: HTMLElement) => void}} chat
   */
  constructor(text, chat) {
    this.text = text;
    this.chat = chat;
    /** 会话 → 留下的几条和钉在哪（还没开的新会话记在 `''` 下） @type {Map<string, {report: any, where: string}[]>} */
    this.by = new Map();
    this.session = /** @type {string|null} */ (null);
  }

  /** 看这个会话的：照原来的位置再钉（换会话时正文从头画过，钉的都清掉了）。 @param {string|null} session */
  show(session) {
    this.session = session;
    for (const r of this.by.get(session ?? '') ?? []) this.chat.place(r.where, this.node(r.report, false));
  }

  /** 新会话开了：留下的跟过去。 @param {string|null} from @param {string} to */
  rename(from, to) {
    const got = this.by.get(from ?? '');
    if (!got) return;
    this.by.delete(from ?? '');
    this.by.set(to, [...(this.by.get(to) ?? []), ...got]);
  }

  /** 演示的记一条；是正在看的会话的，钉在这时正文的末尾、淡入，交回 `true`（要滚到露出它）。 @param {string|null} session @param {any} report */
  addLocal(session, report) {
    const key = session ?? '';
    const shown = key === (this.session ?? '');
    const node = this.node(report, true);
    const where = shown ? this.chat.anchor(node) : '';
    this.by.set(key, [...(this.by.get(key) ?? []), { report, where }]);
    return shown;
  }

  /** 卡片（蓝图「确认和提问」第 6 条）：提问一道一块；不允许一行；取消的照别的提示行，暗色一行。 @param {any} r @param {boolean} fresh */
  node(r, fresh) {
    const t = this.text;
    const cls = fresh ? '.is-fresh' : '';
    if (r.type === 'answered') {
      return h(`div.asking-report.asking-card${cls}`,
        h('div.asking-card-head', icon('message-circle'), h('span', t('answered_title') + (r.who ? t('answered_who', { who: r.who }) : ''))),
        h('div.asking-card-rows', r.rows.map((row) => h('div.asking-card-row',
          h('div.asking-card-label', row.label),
          row.answer?.text ? h('div.asking-card-answer', row.answer.text) : row.answer ? null : h('div.asking-card-answer.is-missing', t('unanswered')),
          row.answer?.notes ? h('div.asking-card-notes', t('notes_line', { notes: row.answer.notes })) : null))));
    }
    if (r.type === 'denied') {
      return h(`div.asking-report.asking-card.is-denied${cls}`, h('div.asking-card-line', icon('circle-x'),
        h('span.asking-card-denied', t('denied')), r.reason ? h('span.asking-card-reason', t('denied_reason', { reason: r.reason })) : null));
    }
    return h(`div.asking-report.is-cancelled${cls}`, h('span.asking-dot', '●'), t(r.kind === 'ask' ? 'cancelled_question' : 'cancelled_approval'));
  }
}
