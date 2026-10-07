// @ts-check
//! 了结以后留下的（蓝图 `web.md`「确认和提问」第 6 条）怎么画（`node`）：由正文那一层照日志排、经挂载位 `chat.item` 交给这里画
//! （`index.js`），夹在她这一轮里。留什么由 `model.js` 的 `report`、`reportOf` 算，这里只画。

import { h, icon } from '../../src/lib/dom.js';

export class Reports {
  /** @param {(key: string, fields?: Record<string, any>) => string} text */
  constructor(text) {
    this.text = text;
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
