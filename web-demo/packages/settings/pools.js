// @ts-check
//! 模型池（蓝图 `web.md`「设置页」第 14 条）：一个池一张卡片：头上 `@名字`、调用方式（固定、轮换）、「＋ 添加」（从还没进来的模型里
//! 勾）、删除；下面成员一行一个（序号、名字、哪一家，悬停露出往上、往下、✕）；最后一张「新建模型池」写名字回车建好。改了当场存，
//! 写进个人设置。

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
  const byRef = new Map(all.map((r) => [r.ref, r]));
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
    // 成员一行一个：序号、名字（池里重了的写模型名分开）、哪一家；悬停露出往上、往下、✕。配置里写着、认不出的照原样写
    const known = now.map((ref) => byRef.get(ref) ?? { ref, name: ref, model: ref, provider: '' });
    const dup = duplicates(known.map((r) => r.name));
    // 第一行的往上、最后一行的往下留着位置不露（一列一列对齐）
    const tool = (label, name, onclick, off) => h(`button.set-member-tool${off ? '.is-off' : ''}`, { type: 'button', title: label, 'aria-label': label, disabled: off ? true : null, onclick }, icon(name));
    const rows = known.map((r, i) => h('div.set-member',
      h('span.set-member-n', String(i + 1)),
      h('span.set-model-name', { title: r.ref }, r.name, dup.has(r.name) ? h('code.set-model-id', r.model) : null),
      h('span.set-pick-provider', r.provider),
      h('span.set-member-tools',
        tool(t('pool_up'), 'arrow-up', () => setMembers(moveMember(now, i, -1)), i === 0),
        tool(t('pool_down'), 'arrow-down', () => setMembers(moveMember(now, i, 1)), i === now.length - 1),
        tool(t('pool_remove'), 'x', () => setMembers(now.filter((_, n) => n !== i))))));
    // 「＋ 添加」在头上：浮出选模型的小窗，能勾好几个，照勾的先后接在最后
    const add = h('button.set-btn.is-small', { type: 'button' }, icon('plus'), t('pool_add'));
    add.addEventListener('click', () => pickModels(dialog, add, all.filter((r) => !now.includes(r.ref)), (picked) => setMembers([...now, ...picked])));
    return h('div.set-pool',
      h('div.set-pool-head', h('strong', `@${pool.name}`), seg, add,
        removal.length ? h('button.set-reset.is-shown', { type: 'button', title: t('pool_delete'), 'aria-label': t('pool_delete'), onclick: async () => report(await dialog.saveMany(layer, removal)) }, icon('trash-2')) : null),
      rows.length ? h('div.set-members', rows) : h('span.set-muted', t('pool_empty')));
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
