// @ts-check
//! 起内核（蓝图 `web/architecture.md`「分层」「发行版」）：读资源、连核心、握手，起编进内核的服务，再照发行版和个人那一层
//! 加载软件包。内核自己站得住：一个包都起不来时，页面上列出哪个包卡在哪，不白屏。
//!
//! 编进内核的服务（宏内核：决定快慢、彼此紧挨着的放在一起，像编进内核的驱动）：
//! `core` 连核心（线是宿主给的）、`sessions` 会话仓库、`host` 宿主（平台有关的：这个页面登录成的账号、起桥的目录和家目录、
//! 附件从哪来、外链、剪贴板；蓝图「宿主」）、`page` 页面的根、`slots` 挂载位、`seams` 职能、`packages` 软件包（状态、停用、
//! 启用、改配置）、`storage` 这台设备上存的东西（键带账号，「多用户、多终端」）。职能选出来的提供者也当服务发出去（名字就是
//! 职能的名字），用它的包跟着重来。
//!
//! 宿主现在只有浏览器一份（`src/host/browser.js`）；桌面端（Tauri）加一份，在这里照所在的平台选。

import { Registry, Fiber } from './context.js';
import { Slots } from './slots.js';
import { Seams } from './seams.js';
import { Loader, rows } from './loader.js';
import { loadResources, res, t } from '../util/res.js';
import { Connection } from '../core/connection.js';
import { Store } from '../core/store.js';
import { useHost } from '../core/host.js';
import { browserHost } from '../host/browser.js';
import { accountStorage } from './storage.js';
import { useIcons } from '../lib/dom.js';
import { KERNEL_SERVICES } from './services.js';

/** 个人那一层（停用、启用、改配置）存在这台设备上的名字（带上账号）；核心有了个人设置以后搬进账号的家目录。 */
const USER_LAYER = 'packages';
/** 原来不分账号时的名字：第一次读时搬到这个账号名下 */
const LEGACY = { [USER_LAYER]: 'miyu.packages' };

/**
 * 起页面。
 * @param {HTMLElement} root 页面的根
 */
export async function boot(root) {
  const base = new URL('../../', import.meta.url);
  await loadResources(new URL('resources/', base));
  useIcons(res.lucide.icons);
  const host = browserHost();
  if (!host) throw new Error(t('no_bridge'));
  // 旧代码（src/ui）经它拿地址、开外链、写剪贴板
  useHost(host);
  // 断了自己重连（蓝图 `web.md`「连核心」第 1 条）：连上以后重新握手、补上漏掉的，见下面 `onReopen`
  const conn = new Connection(host.channel, res.layout.reconnect_ms);
  await conn.connect().catch((err) => {
    throw new Error(err.message === 'bridge' ? t('no_bridge') : err.message);
  });
  // 还没有确认的抽屉：握手时说没人能当场确认，要确认的那一步核心当场拒绝（照 TUI 演示）
  const greeting = { protocol: [1, 1], head: { kind: 'web', version: '0.0.0' }, locale: 'zh-CN', caps: { input: false } };
  const hello = await conn.request('hello', greeting);
  // 这个页面登录成哪个账号：这台设备上存的按它分开（「多用户、多终端」第 5 条）
  const account = hello?.account ?? 'admin';
  const storage = accountStorage(host.store, account, LEGACY);
  // 新会话在哪个目录里干活：桥报的起桥的目录（绝对路径）
  const info = await conn.request('web.info', {});
  // 给人看的字（工具的显示名、结果那一句）：桥照资源目录读好的；拿不到的照工具名写
  res.human = await conn.request('web.human', { language: 'zh' }).catch((err) => {
    console.error(`拿不到给人看的字：${err.message}`);
    return { tools: {}, said: {} };
  });
  const store = new Store(conn);
  await store.boot();
  // 断了又连上了（核心重启过）：重新握手，读进来了的会话重新订阅、补上漏掉的，断着时别处开的会话接上
  conn.onReopen(() => {
    conn.request('hello', greeting)
      .then(() => store.resume())
      .catch((err) => console.error(`重连以后接不上：${err.message}`));
  });

  const registry = new Registry();
  const slots = new Slots();
  const seams = new Seams();
  const distro = await fetch(new URL('distro.json', base)).then((r) => r.json());
  /** 清单：读一次记着（停了的包也要列名字、设置项） */
  const manifests = new Map();
  const manifestOf = async (id) => {
    if (!manifests.has(id)) {
      const got = await fetch(new URL(`packages/${id}/manifest.json`, base));
      if (!got.ok) throw new Error(`找不到软件包 ${id}（${got.status}）`);
      manifests.set(id, await got.json());
    }
    return manifests.get(id);
  };
  const loader = new Loader(registry, {
    load: async (id) => {
      const manifest = await manifestOf(id);
      const module = await import(new URL(`packages/${id}/index.js`, base).href);
      return { manifest, apply: module.apply };
    },
    // 包的样式跟着包挂上、撤下
    styles: (id, files) => {
      const links = files.map((f) => {
        const link = document.createElement('link');
        link.rel = 'stylesheet';
        link.href = new URL(`packages/${id}/${f}`, base).href;
        link.dataset.package = id;
        document.head.append(link);
        return link;
      });
      return () => links.forEach((l) => l.remove());
    },
  });
  const sync = () => loader.sync(rows(distro.packages, storage.get(USER_LAYER, {})));

  // 内核自己：编进内核的服务
  const kernel = new Fiber(registry, {
    manifest: { id: 'kernel' },
    apply: (ctx) => {
      ctx.provide('core', conn);
      ctx.provide('sessions', store);
      ctx.provide('host', {
        kind: host.kind,
        account,
        cwd: info.cwd,
        home: info.home ?? null,
        files: host.files,
        open: host.open,
        clipboard: host.clipboard,
      });
      // 页面里的外链、下载：浏览器照原样，桌面端改走系统的浏览器、存文件的对话框
      ctx.effect(() => host.intercept(root));
      ctx.provide('page', { root });
      ctx.provide('slots', slots);
      ctx.provide('seams', seams);
      ctx.provide('storage', storage);
      ctx.provide('packages', {
        /** 每个包：状态，加上清单里的名字、种类（读不到清单的只有编号） */
        list: async () => Promise.all(loader.status().map(async (s) => ({ ...s, manifest: await manifestOf(s.id).catch(() => null) }))),
        status: () => loader.status(),
        /** 改个人那一层给一个包的补丁（停用、启用、改配置），当场对账 */
        set: async (id, patch) => {
          const user = storage.get(USER_LAYER, {});
          const had = user[id] ?? {};
          // 配置按项合：改一项不冲掉别的项；值写 null 的是改回出厂（删掉这一项）
          const config = patch.config ? Object.fromEntries(Object.entries({ ...had.config, ...patch.config }).filter(([, v]) => v !== null)) : had.config;
          user[id] = { ...had, ...patch, ...(config ? { config } : {}) };
          storage.set(USER_LAYER, user);
          await sync();
        },
      });
    },
  }, {});
  kernel.start();
  const missing = KERNEL_SERVICES.filter((name) => !registry.has(name));
  if (missing.length) throw new Error(`内核少提供了 ${missing.join('、')}`);
  // 职能选出来的提供者当服务发出去
  seams.watch((name, choice) => {
    if ('provider' in choice) registry.set(name, choice.provider, kernel);
    else registry.unset(name, kernel);
  });
  seams.prefer(distro.seams ?? {});

  await sync();
  if (!root.childElementCount) showStatus(root, loader.status());
}

/** 一个包都没画出东西：列出每个包在哪一步，不白屏。 */
function showStatus(root, status) {
  const list = document.createElement('ul');
  list.className = 'kernel-status';
  for (const s of status) {
    const li = document.createElement('li');
    const why = s.reason ?? (s.missing.length ? `等着：${s.missing.join('、')}` : '');
    li.textContent = `${s.id}：${s.state}${why ? `（${why}）` : ''}`;
    list.append(li);
  }
  root.replaceChildren(list);
}
