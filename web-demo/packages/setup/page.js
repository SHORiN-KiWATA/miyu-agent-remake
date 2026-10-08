// @ts-check
//! 设置页的「人格」「预设」两页（蓝图 `web.md`「人格、预设、工作区」第 6 条；挂进设置页的 `settings.section`）：打开时先重读列表和默认的
//! （通用页刚改过默认的这里跟着变），一个一块：名字、说明，默认的那个标「默认」；文件写错的那一块写原因。
//! 照「用户来这一页要做什么」画（2026-10-08 项目主人：来自哪一层、以谁为底、编号、分语言的名字都是核心怎么存，用户用不上，不显示）。
//! 改、新建的详情另做（P-3，等样板定了）。

import { h, replace } from '../../src/lib/dom.js';
import { personaName, presetName } from './model.js';

/** 先空着、读完再填的一页；读不了写原因。 @param {any} ctx @param {(el: HTMLElement) => Promise<void>} fill */
function later(ctx, fill) {
  const el = h('div.setup-cards', h('p.setup-empty', ctx.text('page.loading')));
  fill(el).catch((err) => replace(el, h('p.setup-empty.is-bad', err?.message ?? String(err))));
  return el;
}

/** 一块：名字、说明、小标签。 */
function card(title, summary, tags) {
  return h('div.setup-card', h('h4', title), summary ? h('p', summary) : null, tags.length ? h('div.setup-tags', tags.map((x) => h('span.setup-tag', x))) : null);
}

/** @param {any} ctx @param {import('./catalog.js').Catalog} catalog @returns {HTMLElement} */
export function personaPage(ctx, catalog) {
  const t = (key, fields) => ctx.text(key, fields);
  return later(ctx, async (el) => {
    await catalog.load();
    const cards = await Promise.all((catalog.personas ?? []).map(async (p) => {
      if (p.problem) return h('div.setup-card.is-bad', h('h4', p.persona), h('p.setup-card-problem', p.problem));
      return card(personaName(p), p.summary, p.persona === catalog.personaDefault ? [t('page.default')] : []);
    }));
    replace(el, cards.length ? cards : h('p.setup-empty', t('page.none')), h('p.setup-note', t('page.note')));
  });
}

/** @param {any} ctx @param {import('./catalog.js').Catalog} catalog @returns {HTMLElement} */
export function presetPage(ctx, catalog) {
  const t = (key, fields) => ctx.text(key, fields);
  return later(ctx, async (el) => {
    await catalog.load();
    const fallback = catalog.presetDefault ?? 'full';
    const cards = (catalog.presets ?? []).map((p) => (p.problem
      ? h('div.setup-card.is-bad', h('h4', p.preset), h('p.setup-card-problem', p.problem))
      : card(presetName(p), p.summary, p.preset === fallback ? [t('page.default')] : [])));
    replace(el, cards.length ? cards : h('p.setup-empty', t('presets.none')), h('p.setup-note', t('presets.note')));
  });
}
