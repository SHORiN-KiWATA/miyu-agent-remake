// @ts-check
//! 换模型的菜单（蓝图 `web.md`「换模型的菜单」，照 Claude 网页端的模型菜单）：照 `model.list` 排出模型和模型池两页；一行两行字
//! （名字、小字供应商或池的分法和成员）；现在用着的打勾；用不了的写原因、不能选。框下面那一截怎么拆。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadRes } from './support.js';
import { menuOf, footerOf, effortLevels, effortRows, effortLabel, effortOf, effortChange, defaultModelChange } from '../src/model/model-menu.js';

loadRes();

const model = (provider, name, more = {}) => ({ model: name, ref: `${provider}/${name}`, facts: {}, state: 'ok', ...more });
const LIST = {
  providers: [
    { id: 'dev', models: [model('dev', 'cline-pass/deepseek-v4.1-flash'), model('dev', 'deepseek-v4-pro')] },
    { id: 'bigmodel', models: [model('bigmodel', 'glm-5.3-flash', { state: 'cooling', until: '2026-10-01T14:41:00.000Z', class: 'rate_limited' })] },
    { id: 'anthropic', models: [model('anthropic', 'claude-sonnet-5', { state: 'no_key' })] },
  ],
  pools: [
    { name: 'duo', strategy: 'pin', models: ['bigmodel/glm-5.3-flash', 'dev/cline-pass/deepseek-v4.1-flash'], subagent: true, description: 'Two fast models.' },
    { name: 'spread', strategy: 'rotate', models: ['dev/deepseek-v4-pro'], subagent: false, description: '' },
  ],
  uses: { chat: 'dev/cline-pass/deepseek-v4.1-flash', vision: null },
};

test('模型那一页：一个模型一行，上面模型名、下面供应商，别的不写；现在用着的打勾；用不了的不能选、写原因', () => {
  const { models } = menuOf(LIST, 'dev/deepseek-v4-pro');
  assert.deepEqual(models.map((r) => [r.ref, r.title, r.desc, r.current, r.usable, r.why]), [
    ['dev/cline-pass/deepseek-v4.1-flash', 'cline-pass/deepseek-v4.1-flash', 'dev', false, true, ''],
    ['dev/deepseek-v4-pro', 'deepseek-v4-pro', 'dev', true, true, ''],
    ['bigmodel/glm-5.3-flash', 'glm-5.3-flash', 'bigmodel', false, false, '冷却到 14:41'],
    ['anthropic/claude-sonnet-5', 'claude-sonnet-5', 'anthropic', false, false, '没设 key'],
  ]);
});

test('模型池那一页：@名字，下面写分法和它的模型（照先后，只写模型名）；没配池的是空的', () => {
  const { pools } = menuOf(LIST, '@duo');
  assert.deepEqual(pools.map((r) => [r.ref, r.title, r.desc, r.current, r.usable]), [
    ['@duo', '@duo', '出错换下一个 · glm-5.3-flash、cline-pass/deepseek-v4.1-flash', true, true],
    ['@spread', '@spread', '轮流用 · deepseek-v4-pro', false, true],
  ]);
  assert.deepEqual(menuOf({ ...LIST, pools: [] }, null).pools, []);
});

test('还没有列表（问着、问不到）：两页都是空的', () => {
  assert.deepEqual(menuOf(null, null), { models: [], pools: [] });
});

test('框下面那一截：引用照第一个 / 拆成模型名和供应商（模型名里可以带 /）；池照原样写、不写供应商', () => {
  assert.deepEqual(footerOf('dev/cline-pass/deepseek-v4.1-flash'), { model: 'cline-pass/deepseek-v4.1-flash', endpoint: 'dev' });
  assert.deepEqual(footerOf('@duo'), { model: '@duo', endpoint: null });
  assert.deepEqual(footerOf('cheap'), { model: 'cheap', endpoint: null });
});

test('思考强度有哪几档：照这个模型的资料（facts.reasoning）；池、认不出的、没写的都没有', () => {
  const list = { ...LIST, providers: [{ id: 'dev', models: [
    model('dev', 'deepseek-v4-pro', { facts: { reasoning: { value: ['max', 'low', 'off', 'high'] } } }),
    model('dev', 'plain'),
  ] }] };
  assert.deepEqual(effortLevels(list, 'dev/deepseek-v4-pro'), ['max', 'low', 'off', 'high']);
  assert.deepEqual(effortLevels(list, 'dev/plain'), []);
  assert.deepEqual(effortLevels(list, '@duo'), []);
  assert.deepEqual(effortLevels(null, 'dev/deepseek-v4-pro'), []);
});

test('思考强度的子菜单：默认一直有，别的只列这个模型有的那几档（这个模型没有的不列），照 关、默认、极低、低、中、高、更高、最高 排；只有开关的是 关、默认、开；认不出的名字照原样排在后面；现在那一档打勾', () => {
  const rows = effortRows(['max', 'weird', 'low', 'off', 'high'], 'high');
  assert.deepEqual(rows.map((r) => [r.level, r.title, r.current]), [
    ['off', '关', false], [null, '默认', false], ['low', '低', false], ['high', '高', true], ['max', '最高', false], ['weird', 'weird', false],
  ]);
  assert.deepEqual(effortRows(['off', 'on'], null).map((r) => [r.title, r.current]), [['关', false], ['默认', true], ['开', false]]);
  assert.deepEqual(effortRows([], null).map((r) => r.title), ['默认'], '一档都没报的：只有默认');
  assert.equal(effortLabel('xhigh'), '更高');
  assert.equal(effortLabel(null), '默认');
});

test('现在那一档照 model.list 的 facts.effort：个人设置里写了的（来源是配置的 personal 那一层）照写；系统配置的、没写的就是默认；键名照抄核心给的', () => {
  const list = { providers: [{ id: 'dev', models: [
    model('dev', 'mine', { facts: { effort: { value: 'high', from: 'config', layer: 'personal', file: 'home/admin/settings.toml', line: 4, key: 'providers.dev.models.mine.effort' } } }),
    model('dev', 'sys', { facts: { effort: { value: 'low', from: 'config', layer: 'system', file: 'system/config.toml', line: 9, key: 'providers.dev.models.sys.effort' } } }),
    model('dev', 'none', { facts: { effort: { value: null, from: 'default', key: 'providers.dev.models."v4.1".effort' } } }),
    model('dev', 'old'),
  ] }] };
  assert.deepEqual(effortOf(list, 'dev/mine'), { level: 'high', key: 'providers.dev.models.mine.effort' });
  assert.deepEqual(effortOf(list, 'dev/sys'), { level: null, key: 'providers.dev.models.sys.effort' });
  assert.deepEqual(effortOf(list, 'dev/none'), { level: null, key: 'providers.dev.models."v4.1".effort' });
  assert.deepEqual(effortOf(list, 'dev/old'), { level: null, key: null }, '核心没给键名的：选不了');
  assert.deepEqual(effortOf(list, '@duo'), { level: null, key: null });
  assert.deepEqual(effortOf(null, 'dev/mine'), { level: null, key: null });
});

test('选了一档写进个人设置（config.set）；选默认的删掉个人这一项', () => {
  assert.deepEqual(effortChange('providers.dev.models.mine.effort', 'off'),
    { layer: 'personal', changes: [{ key: 'providers.dev.models.mine.effort', value: 'off' }] });
  assert.deepEqual(effortChange('providers.dev.models.mine.effort', null),
    { layer: 'personal', changes: [{ key: 'providers.dev.models.mine.effort', unset: true }] });
});

test('开发端点那个 DeepSeek 的档位（off、low、high、max）；目录里叫 default 的那一档照原样写', () => {
  assert.deepEqual(effortRows(['off', 'low', 'high', 'max'], null).map((r) => r.title), ['关', '默认', '低', '高', '最高']);
  assert.deepEqual(effortRows(['default', 'low'], 'default').map((r) => [r.level, r.title, r.current]),
    [[null, '默认', false], ['low', '低', false], ['default', 'default', true]]);
});

test('手动选的模型记成新会话的默认（2026-10-02 项目主人定）：写个人设置的 models.chat，模型、池照原样', () => {
  assert.deepEqual(defaultModelChange('dev/deepseek-v4-pro'), { layer: 'personal', changes: [{ key: 'models.chat', value: 'dev/deepseek-v4-pro' }] });
  assert.deepEqual(defaultModelChange('@duo'), { layer: 'personal', changes: [{ key: 'models.chat', value: '@duo' }] });
});
