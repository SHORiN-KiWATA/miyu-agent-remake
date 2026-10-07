// @ts-check
//! 设置页的「人格」页（蓝图 `web.md`「人格、预设、工作区」第 6 条；挂进设置页的 `settings.section`）：一个人格一块，名字、编号、说明，
//! 下面一排小标签：是不是默认的、有示范对话的写几轮（来自哪一层不写，2026-10-07 项目主人：没必要）；文件写错的那一块写原因。
//! 只看，新建、修改等核心 P-3。
//! 列表照 `persona.list`，每一块的细节照 `persona.get`（读完再填）。

import { h, replace } from '../../src/lib/dom.js';
import { personaName } from './model.js';

/**
 * @param {any} ctx
 * @param {() => string|null} fallback 配置项 `persona.default`
 * @returns {HTMLElement}
 */
export function personaPage(ctx, fallback) {
  const t = (key, fields) => ctx.text(key, fields);
  const el = h('div.setup-cards', h('p.setup-empty', t('page.loading')));
  const fill = async () => {
    const { personas } = await ctx.core.request('persona.list', {});
    const cards = await Promise.all((personas ?? []).map(async (p) => {
      if (p.problem) return h('div.setup-card.is-bad', h('h4', p.persona), h('p.setup-card-problem', p.problem));
      const got = await ctx.core.request('persona.get', { persona: p.persona }).catch(() => null);
      const tags = [
        p.persona === fallback() ? t('page.default') : null,
        got?.examples ? t('page.examples', { count: got.examples }) : null,
      ].filter(Boolean);
      return h('div.setup-card',
        h('h4', personaName(p), p.name ? h('code', p.persona) : null),
        p.summary ? h('p', p.summary) : null,
        tags.length ? h('div.setup-tags', tags.map((x) => h('span.setup-tag', x))) : null);
    }));
    replace(el, cards.length ? cards : h('p.setup-empty', t('page.none')), h('p.setup-note', t('page.note')));
  };
  fill().catch((err) => replace(el, h('p.setup-empty.is-bad', err?.message ?? String(err))));
  return el;
}
