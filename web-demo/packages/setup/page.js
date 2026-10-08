// @ts-check
//! 设置页的「人格」「预设」两页（蓝图 `web.md`「人格、预设、工作区」第 6 条；挂进设置页的 `settings.section`）：打开时先重读列表和默认的
//! （通用页刚改过默认的这里跟着变），一个一块：名字、编号、说明，下面一排小标签。只看，新建、修改等核心 P-3。
//! - 人格：小标签是「默认」、有示范对话的写几轮（来自哪一层不写，2026-10-07 项目主人：没必要）；文件写错的那一块写原因。
//! - 预设：小标签是「默认」、预设写的默认人格；下面照 `preset.get` 的 `switches` 一个软件一个开关（项目主人定：预设是全部功能的开关），
//!   开关先只看、点不动（改等 P-3）；写了没装的（`missing`）写「没安装」。

import { h, replace } from '../../src/lib/dom.js';
import { personaName, presetName } from './model.js';

/** 先空着、读完再填的一页；读不了写原因。 @param {any} ctx @param {(el: HTMLElement) => Promise<void>} fill */
function later(ctx, fill) {
  const el = h('div.setup-cards', h('p.setup-empty', ctx.text('page.loading')));
  fill(el).catch((err) => replace(el, h('p.setup-empty.is-bad', err?.message ?? String(err))));
  return el;
}

/** 一块：名字（有显示名的后面暗色写编号）、说明、小标签，再接 `more`。 */
function card(title, id, named, summary, tags, ...more) {
  return h('div.setup-card',
    h('h4', title, named ? h('code', id) : null),
    summary ? h('p', summary) : null,
    tags.length ? h('div.setup-tags', tags.map((x) => h('span.setup-tag', x))) : null,
    ...more);
}

/** @param {any} ctx @param {import('./catalog.js').Catalog} catalog @returns {HTMLElement} */
export function personaPage(ctx, catalog) {
  const t = (key, fields) => ctx.text(key, fields);
  return later(ctx, async (el) => {
    await catalog.load();
    const cards = await Promise.all((catalog.personas ?? []).map(async (p) => {
      if (p.problem) return h('div.setup-card.is-bad', h('h4', p.persona), h('p.setup-card-problem', p.problem));
      const got = await ctx.core.request('persona.get', { persona: p.persona }).catch(() => null);
      const tags = [p.persona === catalog.personaDefault ? t('page.default') : null, got?.examples ? t('page.examples', { count: got.examples }) : null].filter(Boolean);
      return card(personaName(p), p.persona, !!p.name, p.summary, tags);
    }));
    replace(el, cards.length ? cards : h('p.setup-empty', t('page.none')), h('p.setup-note', t('page.note')));
  });
}

/** @param {any} ctx @param {import('./catalog.js').Catalog} catalog @returns {HTMLElement} */
export function presetPage(ctx, catalog) {
  const t = (key, fields) => ctx.text(key, fields);
  const personaOf = (id) => {
    const p = catalog.personas?.find((x) => x.persona === id);
    return p ? personaName(p) : id;
  };
  return later(ctx, async (el) => {
    await catalog.load();
    const fallback = catalog.presetDefault ?? 'full';
    const cards = await Promise.all((catalog.presets ?? []).map(async (p) => {
      if (p.problem) return h('div.setup-card.is-bad', h('h4', p.preset), h('p.setup-card-problem', p.problem));
      const got = await catalog.preset(p.preset);
      const tags = [p.preset === fallback ? t('page.default') : null, got?.default_persona ? t('presets.persona', { name: personaOf(got.default_persona) }) : null].filter(Boolean);
      const switches = Object.entries(got?.switches ?? {}).map(([id, on]) => h('div.setup-switch',
        h('code', id),
        h(`span.setup-toggle${on ? '.is-on' : ''}`, { role: 'switch', 'aria-checked': String(!!on), 'aria-disabled': 'true', title: t('presets.readonly') }, h('i'))));
      const missing = got?.missing?.length ? h('p.setup-missing', t('presets.missing', { list: got.missing.join('、') })) : null;
      return card(presetName(p), p.preset, !!p.name, p.summary, tags, switches.length ? h('div.setup-switches', switches) : null, missing);
    }));
    replace(el, cards.length ? cards : h('p.setup-empty', t('presets.none')), h('p.setup-note', t('presets.note')));
  });
}
