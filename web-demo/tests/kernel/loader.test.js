// @ts-check
//! 加载器（蓝图 `web/architecture.md`「发行版」）：发行版和个人那一层合成包的列表；照 `id` 对账：新的加载、没了的撤回、
//! 停用的撤回、配置变了重装；清单自己有毛病的只它故障；状态说得出每个包在哪一步。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Registry } from '../../src/kernel/context.js';
import { Loader, rows } from '../../src/kernel/loader.js';

/** 假的包：清单、入口都在内存里；记下谁起了、谁撤了、用的什么配置。 */
function world() {
  const log = [];
  const packages = {
    theme: { manifest: { id: 'theme', settings: {} }, apply: (ctx) => { log.push('theme 起'); ctx.effect(() => () => log.push('theme 撤')); ctx.provide('theme', { name: 'x' }); } },
    rail: {
      manifest: { id: 'rail', inject: ['theme'], settings: { gutter: { type: 'number', default: 40, min: 0 } } },
      apply: (ctx) => { log.push(`rail 起 ${ctx.config.gutter}`); ctx.effect(() => () => log.push('rail 撤')); },
    },
    broken: { manifest: { id: 'broken', settings: { n: { type: 'number', default: 'x' } } }, apply: () => log.push('broken 起') },
  };
  const loader = new Loader(new Registry(), {
    load: async (id) => {
      const p = packages[id];
      if (!p) throw new Error(`没有 ${id} 这个包`);
      return p;
    },
    styles: () => () => {},
  });
  return { log, loader };
}

test('发行版和个人那一层合成列表：个人按 id 停用、改配置', () => {
  const distro = [{ id: 'theme' }, { id: 'rail', config: { gutter: 30 } }];
  const user = { rail: { disabled: true }, theme: { config: { x: 1 } } };
  assert.deepEqual(rows(distro, user), [
    { id: 'theme', disabled: false, distro: {}, user: { x: 1 } },
    { id: 'rail', disabled: true, distro: { gutter: 30 }, user: {} },
  ]);
});

test('照 id 对账：加载、配置合层、停用撤回、改配置重装、拿掉撤回', async () => {
  const { log, loader } = world();
  await loader.sync([{ id: 'rail', disabled: false, distro: { gutter: 30 }, user: {} }, { id: 'theme', disabled: false, distro: {}, user: {} }]);
  assert.deepEqual(log, ['theme 起', 'rail 起 30'], '先起的 rail 等着 theme，theme 来了才起');
  await loader.sync([{ id: 'rail', disabled: false, distro: { gutter: 30 }, user: { gutter: 12 } }, { id: 'theme', disabled: false, distro: {}, user: {} }]);
  assert.deepEqual(log.slice(2), ['rail 撤', 'rail 起 12']);
  await loader.sync([{ id: 'rail', disabled: true, distro: {}, user: {} }, { id: 'theme', disabled: false, distro: {}, user: {} }]);
  assert.deepEqual(log.slice(4), ['rail 撤']);
  await loader.sync([]);
  assert.deepEqual(log.slice(5), ['theme 撤']);
});

test('状态：就绪、等着（缺什么）、故障（为什么）、停了、找不到', async () => {
  const { loader } = world();
  await loader.sync([
    { id: 'rail', disabled: false, distro: {}, user: {} },
    { id: 'broken', disabled: false, distro: {}, user: {} },
    { id: 'ghost', disabled: false, distro: {}, user: {} },
    { id: 'theme', disabled: true, distro: {}, user: {} },
  ]);
  const st = Object.fromEntries(loader.status().map((s) => [s.id, s]));
  assert.equal(st.rail.state, 'pending');
  assert.deepEqual(st.rail.missing, ['theme']);
  assert.equal(st.broken.state, 'failed');
  assert.match(st.broken.reason, /n/);
  assert.equal(st.ghost.state, 'failed');
  assert.match(st.ghost.reason, /ghost/);
  assert.equal(st.theme.state, 'disabled');
});

test('个人那一层某项写错：用下面一层的，状态里写明', async () => {
  const { log, loader } = world();
  await loader.sync([{ id: 'theme', disabled: false, distro: {}, user: {} }, { id: 'rail', disabled: false, distro: {}, user: { gutter: -5 } }]);
  assert.ok(log.includes('rail 起 40'));
  const rail = loader.status().find((s) => s.id === 'rail');
  assert.equal(rail?.errors[0].key, 'gutter');
});

test('只改了 live 的项：当场交给这个包（ctx.config 换成新的、听的收到），不重装；改了别的项照常重装', async () => {
  const log = [];
  const loader = new Loader(new Registry(), {
    load: async () => ({
      manifest: { id: 'theme', settings: { palette: { type: 'text', default: '', applies: 'live' }, size: { type: 'number', default: 1 } } },
      apply: (ctx) => {
        log.push(`起 ${ctx.config.palette || '-'}`);
        ctx.watchConfig(() => log.push(`换成 ${ctx.config.palette}`));
        ctx.effect(() => () => log.push('撤'));
      },
    }),
    styles: () => () => {},
  });
  const row = (user) => [{ id: 'theme', disabled: false, distro: {}, user }];
  await loader.sync(row({}));
  await loader.sync(row({ palette: 'tokyonight' }));
  assert.deepEqual(log, ['起 -', '换成 tokyonight']);
  await loader.sync(row({ palette: 'tokyonight', size: 2 }));
  assert.deepEqual(log.slice(2), ['撤', '起 tokyonight']);
});
