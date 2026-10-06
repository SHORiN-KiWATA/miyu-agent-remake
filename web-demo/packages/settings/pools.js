// @ts-check
//! 模型池（蓝图 `web.md`「设置页」第 14 条）：一个池一张卡片：`@名字`、调用方式（固定、轮换）、成员一个个小块（往前、往后挪，✕ 去掉，
//! 「＋」从还没进来的模型里加）、右上角删除；最后一张「新建模型池」写名字回车建好。改了当场存，写进个人设置。

import { h, icon, replace } from '../../src/lib/dom.js';
import { pickModels } from './picker.js';
import { itemFor, layerFor, validId, keySegment, moveMember, poolRemoval, duplicates, providerName } from './model.js';

/**
 * @param {any} dialog
 * @param {{pools?: any[], providers?: any[]}} list `model.list`
 */
export function drawPools(dialog, list) {
  const ctx = dialog.ctx;
  const t = (k, f) => ctx.text(`models.${k}`, f);
  const all = (list.providers ?? []).flatMap((p) => (p.models ?? []).map((m) => ({ ref: m.ref, model: m.model, name: m.facts?.name?.value ?? m.model, provider: providerName(p) })));
  // 显示名重了的（同一个模型的几条线路）写模型名分开它们
  const dup = duplicates(all.map((r) => r.name));
  const refs = all.map((r) => ({ ref: r.ref, name: dup.has(r.name) ? r.model : r.name }));
  const field = (id, name) => itemFor(dialog.schema, dialog.got, `pools.<id>.${name}`, { id });
  const layer = layerFor(dialog.schema.items.find((i) => i.key === 'pools.<id>.models') ?? { layers: ['personal'] });
  const report = (why) => { if (why) dialog.toast(why); };

  const card = (pool) => {
    const members = field(pool.name, 'models');
    const strategy = field(pool.name, 'strategy');
    const now = /** @type {string[]} */ (members?.entry?.value ?? pool.models ?? []);
    const setMembers = async (next) => members && report(await dialog.save(members, { value: next }));
    const removal = poolRemoval(pool.name, dialog.got, layer);
    const seg = h('div.set-seg', ['pin', 'rotate'].map((s) => h(`button.set-seg-item${pool.strategy === s ? '.is-on' : ''}`, {
      type: 'button',
      onclick: async () => { if (strategy && pool.strategy !== s) report(await dialog.save(strategy, { value: s })); },
    }, t(`strategies.${s}`))));
    const chips = now.map((ref, i) => h('span.set-chip.is-member',
      h('button', { type: 'button', 'aria-label': '←', disabled: i === 0 ? true : null, onclick: () => setMembers(moveMember(now, i, -1)) }, icon('chevron-left')),
      h('span', { title: ref }, refs.find((r) => r.ref === ref)?.name ?? ref),
      h('button', { type: 'button', 'aria-label': '→', disabled: i === now.length - 1 ? true : null, onclick: () => setMembers(moveMember(now, i, 1)) }, icon('chevron-right')),
      h('button', { type: 'button', 'aria-label': '×', onclick: () => setMembers(now.filter((_, n) => n !== i)) }, icon('x'))));
    const add = h('button.set-chip.is-add', { type: 'button', title: t('pool_add'), 'aria-label': t('pool_add') }, icon('plus'));
    // 「＋」：浮出选模型的小窗，能勾好几个，照勾的先后接在最后
    add.addEventListener('click', () => pickModels(dialog, add, all.filter((r) => !now.includes(r.ref)), (picked) => setMembers([...now, ...picked])));
    return h('div.set-pool',
      h('div.set-pool-head', h('strong', `@${pool.name}`), seg,
        removal.length ? h('button.set-reset.is-shown', { type: 'button', title: t('pool_delete'), 'aria-label': t('pool_delete'), onclick: async () => report(await dialog.saveMany(layer, removal)) }, icon('trash-2')) : null),
      h('div.set-chips', chips, add),
      now.length ? null : h('span.set-muted', t('pool_empty')));
  };

  // 新建：这张卡变成写名字的框，回车建好（固定、没有成员），`Esc` 取消
  const create = h('button.set-pool.is-add', { type: 'button' }, icon('plus'), t('new_pool'));
  create.addEventListener('click', () => {
    const error = h('span.set-error');
    const input = /** @type {HTMLInputElement} */ (h('input.set-input', { type: 'text', placeholder: t('pool_name_hint'), spellcheck: 'false', autocomplete: 'off' }));
    const box = h('div.set-pool.is-new', h('div.set-pool-head', h('strong', '@'), input), error);
    const cancel = () => box.replaceWith(create);
    box.addEventListener('set-dismiss', cancel);
    box.classList.add('set-form');
    input.addEventListener('keydown', async (e) => {
      if (e.key !== 'Enter') return;
      const id = input.value.trim();
      if (!validId(id)) return replace(error, ctx.text('models.form.bad_id'));
      if ((list.pools ?? []).some((p) => p.name === id)) return replace(error, ctx.text('models.form.taken_id', { id }));
      const base = `pools.${keySegment(id)}`;
      const why = await dialog.saveMany(layer, [{ key: `${base}.strategy`, value: 'pin', expect: {} }, { key: `${base}.models`, value: [], expect: {} }]);
      if (why) replace(error, why);
    });
    create.replaceWith(box);
    input.focus();
  });
  return h('div.set-pools', (list.pools ?? []).map(card), create);
}
