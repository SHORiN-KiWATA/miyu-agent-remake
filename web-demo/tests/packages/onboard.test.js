// @ts-check
//! 接一家供应商的纯逻辑（蓝图 `web.md`「第一次引导」第 5–7 条，照核心 `cli/setup.md`）：编号怎么起、选一家那一屏的几行和标记、
//! `provider.test` 带什么、存的时候写哪几项、没测成写什么。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { configId, customId, validUrl, cleanUrl, providerRows, testParams, setupChanges, noPools, modelOrder, filterNames, failureOf } from '../../packages/settings/onboard/logic.js';

test('目录里的编号不合写法的换成配置里的写法', () => {
  assert.equal(configId('deepseek'), 'deepseek');
  assert.equal(configId('302ai'), 'p-302ai');
  assert.equal(configId('wafer.ai'), 'wafer-ai');
  assert.equal(configId('moonshotai-cn'), 'moonshotai-cn');
  assert.equal(configId('x'.repeat(40)).length, 32);
});

test('自定义的编号照主机名起，本机的是 local，重了加 -2', () => {
  const none = new Set();
  assert.equal(customId('https://api.example.com/v1', none), 'example');
  assert.equal(customId('https://api.example.co.uk/v1', none), 'example');
  assert.equal(customId('https://302.ai/v1', none), 'p-302');
  assert.equal(customId('http://localhost:11434/v1', none), 'local');
  assert.equal(customId('http://127.0.0.1:1234/v1', none), 'local');
  assert.equal(customId('http://[::1]:8080/v1', none), 'local');
  assert.equal(customId('https://relay.io', none), 'relay');
  assert.equal(customId('https://api.example.com/v1', new Set(['example', 'example-2'])), 'example-3');
});

test('地址要 http(s) 开头，去掉末尾的斜杠', () => {
  assert.equal(validUrl('https://api.example.com/v1'), true);
  assert.equal(validUrl('http://localhost:1234'), true);
  assert.equal(validUrl('api.example.com'), false);
  assert.equal(validUrl('ftp://x'), false);
  assert.equal(validUrl('https://'), false);
  assert.equal(cleanUrl('  https://a.b/v1// '), 'https://a.b/v1');
});

test('选一家：配好的、找到密钥的、用不了的照标，本机的写主机和端口', () => {
  const featured = [
    { id: 'deepseek', name: 'DeepSeek', supported: true },
    { id: 'openai', name: 'OpenAI', supported: true },
    { id: 'anthropic', name: 'Anthropic', supported: false },
    { id: 'opencode', name: 'opencode Zen', supported: true },
  ];
  const detect = {
    keys: [
      { env: 'DEEPSEEK_API_KEY', provider: 'deepseek', supported: true },
      { env: 'ANTHROPIC_API_KEY', provider: 'anthropic', supported: false },
      { env: 'OPENAI_API_KEY', provider: 'openai', supported: true, configured: 'work' },
    ],
    local: [{ provider: 'lmstudio', name: 'LMStudio', base_url: 'http://127.0.0.1:1234/v1', models: ['q'] }, { provider: 'ollama', name: 'Ollama', base_url: 'http://localhost:11434/v1', configured: 'ollama' }],
  };
  const rows = providerRows(featured, detect, new Set(['opencode']));
  assert.deepEqual(rows.common.map((r) => [r.id, r.kind === 'catalog' && r.env, r.configured, r.kind === 'catalog' && r.supported]), [
    ['deepseek', 'DEEPSEEK_API_KEY', null, true],
    ['openai', 'OPENAI_API_KEY', 'work', true],
    ['anthropic', null, null, false],
    ['opencode', null, 'opencode', true],
  ]);
  assert.deepEqual(rows.local.map((r) => [r.id, r.kind === 'local' && r.host, r.configured]), [['lmstudio', '127.0.0.1:1234', null], ['ollama', 'localhost:11434', 'ollama']]);
  assert.deepEqual(providerRows([], null, new Set()), { common: [], local: [] });
});

test('测试的参数：配好的带 provider，别的带 candidate，贴的密钥照 value 交', () => {
  assert.deepEqual(testParams({ kind: 'configured', id: 'deepseek' }, null, null), { provider: 'deepseek' });
  assert.deepEqual(testParams({ kind: 'catalog', catalog: 'deepseek', env: null }, { value: 'sk-1' }, null), { candidate: { catalog: 'deepseek', key: { value: 'sk-1' } } });
  assert.deepEqual(testParams({ kind: 'catalog', catalog: 'deepseek', env: 'DEEPSEEK_API_KEY' }, { env: 'DEEPSEEK_API_KEY' }, null), { candidate: { catalog: 'deepseek', key: { env: 'DEEPSEEK_API_KEY' } } });
  assert.deepEqual(testParams({ kind: 'local', catalog: 'lmstudio' }, null, null), { candidate: { catalog: 'lmstudio' } });
  assert.deepEqual(testParams({ kind: 'custom', driver: 'anthropic', base_url: 'https://r.example.com/v1' }, null, 'm-1'), { candidate: { driver: 'anthropic', base_url: 'https://r.example.com/v1' }, model: 'm-1' });
});

test('存：目录里的一家写密钥，改过写法的另写 catalog，本机的写 local，自定义写接口和地址，没池的建三个', () => {
  const taken = new Set(['example']);
  assert.deepEqual(setupChanges({ kind: 'catalog', catalog: 'deepseek', env: null }, { secret: 'deepseek-abc' }, { chat: 'deepseek-v4', taken }), {
    id: 'deepseek',
    changes: [{ key: 'providers.deepseek.key', value: { secret: 'deepseek-abc' } }, { key: 'models.chat', value: 'deepseek/deepseek-v4' }],
  });
  assert.deepEqual(setupChanges({ kind: 'catalog', catalog: '302ai', env: 'X' }, { env: 'X' }, { taken }).changes, [
    { key: 'providers.p-302ai.catalog', value: '302ai' },
    { key: 'providers.p-302ai.key', value: { env: 'X' } },
  ]);
  assert.deepEqual(setupChanges({ kind: 'local', catalog: 'lmstudio' }, null, { taken }).changes, [{ key: 'providers.lmstudio.local', value: true }]);
  assert.deepEqual(setupChanges({ kind: 'custom', driver: 'anthropic', base_url: 'https://api.example.com/v1' }, null, { chat: 'm', taken }), {
    id: 'example-2',
    changes: [
      { key: 'providers.example-2.driver', value: 'anthropic' },
      { key: 'providers.example-2.base_url', input: 'https://api.example.com/v1' },
      { key: 'models.chat', value: 'example-2/m' },
    ],
  });
  const pooled = setupChanges({ kind: 'configured', id: 'zen' }, null, { chat: 'big', pools: true, taken });
  assert.equal(pooled.id, 'zen');
  assert.deepEqual(pooled.changes.map((c) => c.key), ['models.chat', 'pools.lite.models', 'pools.lite.subagent', 'pools.standard.models', 'pools.standard.subagent', 'pools.flagship.models', 'pools.flagship.subagent']);
  assert.deepEqual(pooled.changes[1], { key: 'pools.lite.models', value: [] });
  assert.equal(noPools({ 'models.chat': {} }), true);
  assert.equal(noPools({ 'pools.fast.models': {} }), false);
});

test('模型的先后：试的那一个排第一，去重；搜索不分大小写', () => {
  assert.deepEqual(modelOrder(['a', 'b', 'c', 'b'], 'c'), ['c', 'a', 'b']);
  assert.deepEqual(modelOrder(['a'], null), ['a']);
  assert.deepEqual(filterNames(['DeepSeek-V4', 'gpt-5'], 'deep'), ['DeepSeek-V4']);
  assert.deepEqual(filterNames(['a', 'b'], '  '), ['a', 'b']);
});

test('没测成：列不出模型的要填模型名，配置错的、别的照分类，带状态码', () => {
  assert.deepEqual(failureOf({ ok: false, stage: 'list', error: { class: 'other', message: 'no models listed' } }), { key: 'onboard.no_list', status: null, needsModel: true, message: 'no models listed' });
  assert.deepEqual(failureOf({ ok: false, stage: 'config', error: { class: 'no_model', message: 'needs base_url' } }).key, 'onboard.config_error');
  assert.deepEqual(failureOf({ ok: false, stage: 'request', error: { class: 'auth', status: 401, message: 'bad key' } }), { key: 'onboard.classes.auth', status: 401, needsModel: false, message: 'bad key' });
  assert.equal(failureOf({ ok: false, stage: 'request', error: { class: 'weird' } }).key, 'onboard.classes.other');
});
