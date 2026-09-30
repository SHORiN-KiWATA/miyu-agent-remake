// @ts-check
//! 了结以后正文末尾留下的（蓝图 `web.md`「确认和提问」第 6 条）：挂进挂载位 `chat.tail`，跟着正在看的会话换；
//! 留什么由 `model.js` 的 `report` 算，这里只画。演示的数据只在这一页里。

import { h, icon, replace } from '../../src/lib/dom.js';

export class Reports {
  /** @param {(key: string, fields?: Record<string, any>) => string} text */
  constructor(text) {
    this.text = text;
    /** 会话 → 留下的几条（还没开的新会话记在 `''` 下） @type {Map<string, any[]>} */
    this.by = new Map();
    this.session = /** @type {string|null} */ (null);
    this.el = h('div.asking-reports');
  }

  /** 看这个会话的。 @param {string|null} session */
  show(session) {
    this.session = session;
    replace(this.el, (this.by.get(session ?? '') ?? []).map((r) => this.node(r, false)));
  }

  /** 新会话开了：留下的跟过去。 @param {string|null} from @param {string} to */
  rename(from, to) {
    const got = this.by.get(from ?? '');
    if (!got) return;
    this.by.delete(from ?? '');
    this.by.set(to, [...(this.by.get(to) ?? []), ...got]);
  }

  /** 记一条；是正在看的会话的，接在后面、淡入，交回 `true`（要滚到露出它）。 @param {string|null} session @param {any} report */
  add(session, report) {
    const key = session ?? '';
    this.by.set(key, [...(this.by.get(key) ?? []), report]);
    if (key !== (this.session ?? '')) return false;
    this.el.append(this.node(report, true));
    return true;
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
