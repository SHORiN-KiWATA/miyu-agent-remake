// @ts-check
//! 软件包 settings（设置页）的纯逻辑：核心的配置清单和最终值合成一页页；写在哪一层、`expect`；带占位的键换成真键；搜索；
//! 清单合法。底料照 2026-10-07 问真核心 `config.schema`、`config.get {all: true}` 回的样子剪的。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { buildPages, noteOf, duplicates, layerFor, expectFor, writtenIn, sourceOf, envRef, inputText, search, pagesWithErrors, splitKey, templateOf, realKey, itemFor, shortCount } from '../../packages/settings/model.js';
import { problems } from '../../src/kernel/config.js';

const schema = {
  pages: [{ id: 'general', name: '通用' }, { id: 'models', name: '模型' }, { id: 'advanced', name: '高级' }],
  groups: [{ id: 'display', name: '显示', page: 'general' }, { id: 'uses', name: '用途', page: 'models' }, { id: 'providers', name: '供应商', page: 'models' }, { id: 'log', name: '运行日志', page: 'advanced' }],
  items: [
    { key: 'ui.language', type: 'option', control: 'select', name: '界面语言', description: '给你看的字用哪种话', page: 'general', group: 'display', common: true, layers: ['system', 'personal'], applies: 'now', default: 'auto' },
    { key: 'models.vision', type: 'reference', control: 'text', name: '看图的模型', description: '', page: 'models', group: 'uses', layers: ['system', 'personal'], applies: 'next_turn' },
    { key: 'models.chat', type: 'reference', control: 'text', name: '主对话的模型', description: '新会话默认用的模型', page: 'models', group: 'uses', common: true, layers: ['system', 'personal'], applies: 'new_session' },
    { key: 'providers.<id>.base_url', type: 'url', control: 'text', name: '地址', description: '', page: 'models', group: 'providers', layers: ['system', 'personal'], applies: 'next_turn' },
    { key: 'providers.<id>.models.<model>.window', type: 'int', control: 'number', name: '窗口', description: '', page: 'models', group: 'providers', layers: ['system', 'personal'], applies: 'next_turn', min: 1, max: 100000000 },
    { key: 'log.level', type: 'option', control: 'select', name: '运行日志的级别', description: '', page: 'advanced', group: 'log', layers: ['system'], applies: 'now', default: 'info' },
  ],
};

const got = {
  items: {
    'ui.language': { layers: [{ origin: { layer: 'default' }, used: true, value: 'auto' }], origin: { layer: 'default' }, value: 'auto' },
    'models.chat': {
      layers: [
        { origin: { file: 'home/admin/settings.toml', layer: 'personal', line: 7 }, used: true, value: 'dev/deepseek-v4.1-flash' },
        { origin: { file: 'system/config.toml', layer: 'system', line: 14 }, used: false, value: 'dev/cline-pass/deepseek-v4.1-flash' },
      ],
      origin: { file: 'home/admin/settings.toml', layer: 'personal', line: 7 },
      value: 'dev/deepseek-v4.1-flash',
    },
    'providers.dev.base_url': { layers: [{ origin: { file: 'system/config.toml', layer: 'system', line: 6 }, used: true, value: { env: 'MIYU_DEV_BASE_URL' } }], origin: { file: 'system/config.toml', layer: 'system', line: 6 }, value: { env: 'MIYU_DEV_BASE_URL' } },
    'log.level': { layers: [{ origin: { layer: 'default' }, used: true, value: 'info' }], origin: { layer: 'default' }, value: 'info' },
  },
  problems: [
    { code: 'env_not_set', key: 'providers.dev.base_url', level: 'warning', message: '引用的环境变量没有设。' },
    { code: 'bad_format', key: 'log.level', level: 'error', message: 'log.level 只能是 error、warn……' },
    { code: 'syntax', level: 'error', message: 'TOML 写法不对' },
  ],
};

test('合成一页页：照 pages、groups 的先后；组里常用的排前面；带占位的项不列；问题挂在那一项上，对不上哪一项的放在「高级」页顶上', () => {
  const pages = buildPages(schema, got);
  assert.deepEqual(pages.map((p) => p.id), ['general', 'models', 'advanced']);
  const models = pages[1];
  assert.deepEqual(models.groups.map((g) => g.id), ['uses'], '供应商那一组只有带占位的项：不列');
  assert.deepEqual(models.groups[0].items.map((i) => i.key), ['models.chat', 'models.vision'], '常用的在前');
  assert.equal(models.groups[0].items[0].entry?.value, 'dev/deepseek-v4.1-flash');
  assert.equal(models.groups[0].items[1].entry, null, '哪一层都没写、没有默认值的：没有最终值');
  const log = pages[2].groups[0].items[0];
  assert.deepEqual(log.problems.map((p) => p.code), ['bad_format']);
  assert.deepEqual(pages[2].problems.map((p) => p.code), ['syntax'], '带占位那几项的问题（env_not_set）不算对不上');
  assert.deepEqual([...pagesWithErrors(pages)], ['advanced']);
});

test('写在哪一层：能写个人设置的写个人设置，只能写系统配置的写系统配置；expect 照这一层写着的，没写的是 {}', () => {
  const chat = schema.items[2];
  const log = schema.items[5];
  assert.equal(layerFor(chat), 'personal');
  assert.equal(layerFor(log), 'system');
  assert.deepEqual(expectFor(got.items['models.chat'], 'personal'), { value: 'dev/deepseek-v4.1-flash' });
  assert.deepEqual(expectFor(got.items['ui.language'], 'personal'), {});
  assert.deepEqual(expectFor(null, 'personal'), {});
  assert.equal(writtenIn(got.items['models.chat'], 'personal'), true, '写过的能恢复默认');
  assert.equal(writtenIn(got.items['ui.language'], 'personal'), false);
});

test('来源、环境变量引用、控件里的字', () => {
  assert.deepEqual(sourceOf(got.items['models.chat']), { file: 'home/admin/settings.toml', layer: 'personal', line: 7 });
  assert.deepEqual(sourceOf(null), { layer: 'none' });
  assert.equal(envRef({ env: 'MIYU_DEV_BASE_URL' }), 'MIYU_DEV_BASE_URL');
  assert.equal(envRef('https://x'), null);
  assert.deepEqual([inputText(null), inputText('10m'), inputText(3), inputText(true), inputText(['a'])], ['', '10m', '3', 'true', '["a"]']);
});

test('搜索：名字、键、说明都算，不分大小写，跨页；空的不搜', () => {
  const pages = buildPages(schema, got);
  assert.deepEqual(search(pages, '模型').map((f) => f.item.key), ['models.chat', 'models.vision']);
  assert.deepEqual(search(pages, 'LOG').map((f) => f.item.key), ['log.level']);
  assert.deepEqual(search(pages, '默认用').map((f) => f.page.id), ['models'], '说明里的字也算');
  assert.deepEqual(search(pages, '  '), []);
});

test('带占位的键：拆键认双引号，真键对得上哪一项，人起的名字照 TOML 写（带斜杠、点的加引号）', () => {
  assert.deepEqual(splitKey('providers.dev.models."cline-pass/deepseek-v4.1-flash".window'), ['providers', 'dev', 'models', 'cline-pass/deepseek-v4.1-flash', 'window']);
  assert.equal(templateOf('providers.dev.base_url', schema.items)?.key, 'providers.<id>.base_url');
  assert.equal(templateOf('ui.language', schema.items), null);
  assert.equal(realKey('providers.<id>.models.<model>.window', { id: 'dev', model: 'cline-pass/deepseek-v4.1-flash' }), 'providers.dev.models."cline-pass/deepseek-v4.1-flash".window');
  assert.equal(realKey('providers.<id>.models.<model>.window', { id: 'dev', model: 'deepseek-v4-pro' }), 'providers.dev.models.deepseek-v4-pro.window');
  const item = itemFor(schema, got, 'providers.<id>.base_url', { id: 'dev' });
  assert.equal(item?.key, 'providers.dev.base_url');
  assert.deepEqual(item?.entry?.value, { env: 'MIYU_DEV_BASE_URL' });
  assert.deepEqual(item?.problems.map((p) => p.code), ['env_not_set']);
  const window = itemFor(schema, got, 'providers.<id>.models.<model>.window', { id: 'dev', model: 'x' }, { value: 1000000 });
  assert.deepEqual([window?.entry, window?.fact?.value], [null, 1000000], '配置里没写的带上目录给的值');
  assert.equal(itemFor(schema, got, 'no.such', {}), null);
});

test('大数写短', () => {
  assert.deepEqual([shortCount(1000000), shortCount(128000), shortCount(131072), shortCount(300), shortCount(undefined)], ['1M', '128k', '131.1k', '300', '']);
});

test('清单合法：出厂值过得了自己的校验；有用到的字', () => {
  const manifest = JSON.parse(readFileSync(new URL('../../packages/settings/manifest.json', import.meta.url), 'utf8'));
  assert.deepEqual(problems(manifest.settings), []);
  assert.ok(manifest.text.zh.layers.factory && manifest.text.zh.models.tabs.pools);
});

test('项少的页并进别的页、个别组挪页；并掉的页不单列，空了的页不列', () => {
  const pages = buildPages(schema, got, { merge: { models: 'general' }, moveGroups: { log: 'general' } });
  assert.deepEqual(pages.map((p) => p.id), ['general', 'advanced'], '模型页并进通用；高级页只剩对不上哪一项的那条问题');
  assert.deepEqual(pages[0].groups.map((g) => g.id), ['display', 'uses', 'log'], '组照 groups 的先后，组名照留');
  assert.deepEqual(pages[1].groups, []);
  const none = buildPages(schema, { items: got.items, problems: [] }, { merge: { models: 'general' }, moveGroups: { log: 'general' } });
  assert.deepEqual(none.map((p) => p.id), ['general'], '一项都没有、也没有问题的页不列');
});

test('名字下面那一行小字：默认又当场生效的整行不出；只写不一样的那部分', () => {
  assert.equal(noteOf(got.items['ui.language'], 'now'), null, '默认值、当场生效');
  assert.deepEqual(noteOf(got.items['ui.language'], 'next_turn'), { source: null, applies: 'next_turn' });
  assert.deepEqual(noteOf(got.items['models.chat'], 'live')?.source?.layer, 'personal');
  assert.equal(noteOf(got.items['models.chat'], 'live')?.applies, null);
  assert.equal(noteOf(null, 'now'), null, '没设的（模型目录给的值）不写来源');
  assert.deepEqual(noteOf({ origin: { layer: 'default', env: 'MIYU_LOG' }, value: 'debug' }, 'now')?.source?.env, 'MIYU_LOG', '环境变量压着的要写');
});

test('同一家里重了的显示名', () => {
  assert.deepEqual([...duplicates(['DeepSeek V4.1 Flash', 'GLM', 'DeepSeek V4.1 Flash', 'DeepSeek V4.1 Flash'])], ['DeepSeek V4.1 Flash']);
  assert.deepEqual([...duplicates(['a', 'b'])], []);
});

test('别的头的项不列：键名以 hide 里的开头的（tui.），连它的问题也不挂；不认识的键照样提示', () => {
  const extra = {
    ...schema,
    groups: [...schema.groups, { id: 'tui', name: '终端界面', page: 'general' }],
    items: [...schema.items, { key: 'tui.startup', type: 'option', control: 'select', name: '启动时打开', description: '', page: 'general', group: 'tui', layers: ['system', 'personal'], applies: 'head_start' }],
  };
  const withBad = { items: got.items, problems: [{ key: 'tui.startup', level: 'error', message: '写错了' }, { key: 'tui.gone', code: 'unknown_key', level: 'warning', message: '没有 tui.gone 这一项' }] };
  const shown = buildPages(extra, withBad, { hide: ['tui.'] });
  assert.deepEqual(shown[0].groups.map((g) => g.id), ['display']);
  assert.deepEqual(shown.flatMap((p) => p.problems).map((p) => p.key), ['tui.gone'], '别的头的项的问题不挂；改了名留下的旧键要提示');
  assert.deepEqual(buildPages(extra, withBad)[0].groups.map((g) => g.id), ['display', 'tui'], '不写 hide 的照列');
});
