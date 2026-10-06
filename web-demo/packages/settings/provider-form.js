// @ts-check
//! 添加、编辑供应商的表（蓝图 `web.md`「设置页」第 14 条，照 `tui.md`「全屏配置页」第 3、7 条）：编号（新建时写）、显示名、地址、
//! 接口、密钥（粘贴或者环境变量）。存的先后：粘贴了密钥的先 `secret.set` 存成新名字，再一条 `config.set` 写别的几项和密钥的引用。
//! 明文只在框里，存完、取消就扔。

import { h, replace } from '../../src/lib/dom.js';
import { validId, secretName, providerChanges, layerFor, envRef, inputText } from './model.js';

/**
 * 画这张表。`dialog.providerForm` 是 `{id}`（编辑）或 `{id: null}`（新建）。
 * @param {any} dialog
 * @param {any[]} providers `model.list` 的供应商
 */
export function providerForm(dialog, providers) {
  const ctx = dialog.ctx;
  const t = (k, f) => ctx.text(`models.form.${k}`, f);
  const editing = dialog.providerForm.id;
  const p = providers.find((x) => x.id === editing) ?? null;
  const template = (field) => dialog.schema.items.find((i) => i.key === `providers.<id>.${field}`);
  const layer = layerFor(template('driver') ?? { layers: ['personal'] });
  const close = () => { dialog.providerForm = null; dialog.drawBody(); };

  const input = (attrs) => /** @type {HTMLInputElement} */ (h('input.set-input', { type: 'text', spellcheck: 'false', autocomplete: 'off', ...attrs }));
  const idField = editing ? h('strong.set-form-fixed', editing) : input({ placeholder: t('id_hint') });
  const nameWritten = p?.name && typeof p.name === 'object' && p.name.from === 'config' ? p.name.value : '';
  // 占位写空着时显示什么：编辑的照现在退到的名字（目录的、编号），新建的是编号
  const fallback = p && typeof p.name === 'object' && p.name.from !== 'config' ? p.name.value : editing;
  const nameField = input({ value: nameWritten, placeholder: fallback ? t('name_empty', { name: fallback }) : t('name_empty_new') });
  // 地址是环境变量引用的：框空着就是不改（照写「来自环境变量 X」当占位）
  const urlEnv = envRef(p?.base_url);
  const urlField = input({ value: urlEnv ? '' : inputText(p?.base_url ?? ''), placeholder: urlEnv ? ctx.text('from_env', { name: urlEnv }) : 'https://…/v1' });
  const drivers = template('driver')?.options ?? [];
  const driverField = /** @type {HTMLSelectElement} */ (h('select.set-input.is-select', drivers.map((o) => h('option', { value: o.value, selected: o.value === (p?.driver ?? drivers[0]?.value) ? true : null }, o.name))));
  // 密钥：粘贴（密码框，编辑时空着是不改）或者环境变量的名字
  const keyRef = p?.keys?.[0]?.ref ?? '';
  let keyKind = keyRef.startsWith('env:') ? 'env' : 'secret';
  const secretField = input({ type: 'password', placeholder: p?.keys?.[0]?.set ? t('key_keep') : t('key_paste') });
  const envField = input({ value: keyRef.startsWith('env:') ? keyRef.slice(4) : '', placeholder: 'DEEPSEEK_API_KEY' });
  const keyBox = h('div.set-form-key');
  const kindButtons = ['secret', 'env'].map((kind) => h('button.set-seg-item', { type: 'button', onclick: () => { keyKind = kind; drawKey(); } }, t(`key_${kind}`)));
  const drawKey = () => {
    kindButtons.forEach((b, n) => b.classList.toggle('is-on', ['secret', 'env'][n] === keyKind));
    replace(keyBox, keyKind === 'secret' ? secretField : envField);
  };
  drawKey();

  const error = h('p.set-error', { hidden: true });
  const fail = (why) => { error.textContent = why; error.hidden = false; saveButton.disabled = false; };
  const saveButton = h('button.set-btn.is-primary', { type: 'button', onclick: async () => {
    error.hidden = true;
    const id = editing ?? /** @type {HTMLInputElement} */ (idField).value.trim();
    if (!editing && !validId(id)) return fail(t('bad_id'));
    if (!editing && providers.some((x) => x.id === id)) return fail(t('taken_id', { id }));
    saveButton.disabled = true;
    let secret = null;
    if (keyKind === 'secret' && secretField.value) {
      secret = secretName(id, Date.now());
      try {
        await ctx.core.request('secret.set', { name: secret, value: secretField.value });
      } catch (err) {
        return fail(err.message);
      }
      secretField.value = '';
    }
    const form = {
      name: nameField.value,
      base_url: urlEnv && !urlField.value.trim() ? null : urlField.value,
      driver: driverField.value,
      key: { kind: /** @type {'secret'|'env'|'keep'} */ (keyKind === 'secret' && !secret ? 'keep' : keyKind), value: envField.value },
    };
    const why = await dialog.saveMany(layer, providerChanges(id, form, dialog.got, layer, secret));
    if (why) return fail(secret ? `${why}\n${t('secret_kept', { name: secret })}` : why);
    // 等模型列表读回来（新建的那一家要在表里）再回到模型那一栏，选中它
    await dialog.modelsPromise;
    dialog.providerForm = null;
    dialog.provider = id;
    dialog.drawBody();
  } }, t('save'));
  const el = h('section.set-form',
    h('h3.set-form-title', editing ? t('edit_title', { name: (typeof p?.name === 'object' ? p.name.value : p?.name) || editing }) : t('new_title')),
    row(t('id'), idField),
    row(t('name'), nameField),
    row(t('url'), urlField),
    row(t('driver'), driverField),
    row(t('key'), h('div.set-form-keyrow', h('div.set-seg', kindButtons), keyBox)),
    error,
    h('div.set-form-buttons', h('button.set-btn', { type: 'button', onclick: close }, t('cancel')), saveButton));
  el.addEventListener('set-dismiss', close);
  queueMicrotask(() => /** @type {HTMLElement} */ (editing ? nameField : idField).focus());
  return el;
}

const row = (label, control) => h('label.set-form-row', h('span', label), control);

/** 「编辑」「添加供应商」按钮：打开表。 */
export const openForm = (dialog, id) => () => { dialog.providerForm = { id }; dialog.modelDetail = null; dialog.drawBody(); };

