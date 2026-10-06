// @ts-check
//! 起内核（蓝图 `web/architecture.md`「分层」「发行版」）：读资源、连核心、握手，起编进内核的服务，再照发行版和个人那一层
//! 加载软件包。内核自己站得住：一个包都起不来时，页面上列出哪个包卡在哪，不白屏。
//!
//! 编进内核的服务（宏内核：决定快慢、彼此紧挨着的放在一起，像编进内核的驱动）：
//! `core` 连核心（线是宿主给的）、`sessions` 会话仓库、`host` 宿主（平台有关的：这个页面登录成的账号、新会话的工作目录（账号的工作区）和家目录、
//! 附件从哪来、外链、剪贴板；蓝图「宿主」）、`page` 页面的根、`slots` 挂载位、`seams` 职能、`packages` 软件包（状态、停用、
//! 启用、改配置）、`storage` 这台设备上存的东西（键带账号，「多用户、多终端」）、`language` 界面语言（内核自己的设置项，
//! 蓝图 `web.md`「界面语言」）。
//!
//! 界面语言先照发行版、浏览器定一种（还不知道账号：连不上桥的那句照它写），握手知道账号以后再照个人那一层定一次，变了重装字。职能选出来的提供者也当服务发出去（名字就是
//! 职能的名字），用它的包跟着重来。
//!
//! 宿主现在只有浏览器一份（`src/host/browser.js`）；桌面端（Tauri）加一份，在这里照所在的平台选。

import { Registry, Fiber, useLanguage } from './context.js';
import { Slots } from './slots.js';
import { Seams } from './seams.js';
import { Loader, rows } from './loader.js';
import { resolve } from './config.js';
import { loadResources, useTexts, res, t } from '../util/res.js';
import { pick, settingOf, options, languageSpec, fromConfig } from './language.js';
import { Connection } from '../core/connection.js';
import { Store } from '../core/store.js';
import { loadHuman } from '../core/human.js';
import { placeOf } from '../model/paths.js';
import { useHost } from '../core/host.js';
import { browserHost } from '../host/browser.js';
import { accountStorage } from './storage.js';
import { useIcons } from '../lib/dom.js';
import { KERNEL_SERVICES } from './services.js';

/** 个人那一层（停用、启用、改配置）存在这台设备上的名字（带上账号）；核心有了个人设置以后搬进账号的家目录。 */
/** JSON-RPC 的「没有这个方法」：连的核心还没有配置的接口 */
const NO_METHOD = -32601;
const USER_LAYER = 'packages';
/** 内核自己的设置项在个人那一层里的编号（和软件包的补丁放在一起；发行版的写在 `distro.json` 的 `kernel`） */
const KERNEL = 'kernel';
/** 原来不分账号时的名字：第一次读时搬到这个账号名下 */
const LEGACY = { [USER_LAYER]: 'miyu.packages' };

/**
 * 起页面。
 * @param {HTMLElement} root 页面的根
 */
export async function boot(root) {
  const base = new URL('../../', import.meta.url);
  const resources = new URL('resources/', base);
  await loadResources(resources);
  useIcons(res.lucide.icons);
  const distro = await fetch(new URL('distro.json', base)).then((r) => r.json());
  // 界面语言：先照发行版、浏览器定一种（个人那一层按账号存，握手以后才读得到）
  const browser = typeof navigator === 'object' ? navigator.languages ?? [] : [];
  let language = pick(settingOf(res.languages, distro.kernel ?? {}, {}), browser, res.languages);
  await useTexts(resources, language);
  document.documentElement.lang = language.tag;
  // 连着桥的时候页面正中写一句，不留一片白（蓝图 `web.md`「连核心」第 9 条）
  root.replaceChildren(Object.assign(document.createElement('div'), { className: 'boot-wait', textContent: t('boot.connecting') }));
  const host = browserHost();
  if (!host) throw new Offline('none');
  // 旧代码（src/ui）经它拿地址、开外链、写剪贴板
  useHost(host);
  // 断了自己重连（蓝图 `web.md`「连核心」第 1 条）：连上以后重新握手、补上漏掉的，见下面 `onReopen`
  const conn = new Connection(host.channel, res.layout.reconnect_ms, host.check);
  // 连不上：问桥口令还对不对，照它写为什么（口令用不了、桥没在跑、桥连不上核心）
  await conn.connect().catch(async (err) => {
    if (err.message !== 'bridge') throw new Offline('core', err.message);
    const key = await host.check();
    throw new Offline(key === 'ok' ? 'core' : key, key === 'ok' ? t('no_bridge') : '');
  });
  root.replaceChildren();
  // 还没有确认的抽屉：握手时说没人能当场确认，要确认的那一步核心当场拒绝（照 TUI 演示）
  const greeting = { protocol: [1, 1], head: { kind: 'web', version: '0.0.0' }, locale: language.tag, caps: { input: false } };
  const hello = await conn.request('hello', greeting);
  // 这个页面登录成哪个账号：这台设备上存的按它分开（「多用户、多终端」第 5 条）
  const account = hello?.account ?? 'admin';
  const storage = accountStorage(host.store, account, LEGACY);
  // 知道账号了：照个人那一层再定一次界面语言；界面的字、收起那一行的写法有一样变了重装字（重连时的握手照新的写）。
  // 核心有配置的：个人设置的 `ui.language` 说了算（蓝图「界面语言」第 5 条，规矩 A，和终端界面一样）；旧核心照这台设备上记的
  let setting = settingOf(res.languages, distro.kernel ?? {}, storage.get(USER_LAYER, {})[KERNEL]?.config ?? {});
  /** 核心有没有配置的接口（`config.get`）：没有的，`/language` 只记在这台设备上 */
  let viaCore = false;
  const readCore = () => conn.request('config.get', { keys: ['ui.language'] }).then((got) => {
    viaCore = true;
    return fromConfig(got);
  }, (err) => {
    if (err?.code !== NO_METHOD) console.error(`读不到个人设置的界面语言：${err.message}`);
    return null;
  });
  const fromCore = await readCore();
  if (fromCore) setting = fromCore;
  const chosen = pick(setting, browser, res.languages);
  if (chosen.code !== language.code || chosen.summary !== language.summary) {
    await useTexts(resources, chosen);
    document.documentElement.lang = chosen.tag;
    greeting.locale = chosen.tag;
  }
  language = chosen;
  // 包的字照它取
  useLanguage(language);
  // 新会话在哪个目录里干活、家目录：握手回应的 `host`（核心施工 W-3；新会话默认在账号的工作区）
  const info = placeOf(hello);
  // 给人看的字（工具的显示名、结果那一句）：问核心的 `human.get`；拿不到的照工具名写
  res.human = await loadHuman(conn, language.code).catch((err) => {
    console.error(`拿不到给人看的字：${err.message}`);
    return { tools: {}, said: {} };
  });
  const store = new Store(conn);
  await store.boot();
  // 断了又连上了（核心重启过）：重新握手，读进来了的会话重新订阅、补上漏掉的，断着时别处开的会话接上
  conn.onReopen(() => {
    conn.request('hello', greeting)
      .then(() => store.resume())
      // 配置的订阅也要等重新握手以后（蓝图「界面语言」第 5 条）
      .then(() => followConfig())
      .catch((err) => console.error(`重连以后接不上：${err.message}`));
  });

  /**
   * 换成这个设置（`auto` 或一种语言）：记进这台设备上的那一层（下次起来、旧核心照它）；界面的字、收起那一行的写法有一样变了，
   * 重新载入页面。交回换完用哪一种、重不重载。
   * @param {string} value
   */
  const applyLanguage = (value) => {
    const user = storage.get(USER_LAYER, {});
    user[KERNEL] = { ...user[KERNEL], config: { ...user[KERNEL]?.config, language: value } };
    storage.set(USER_LAYER, user);
    setting = value;
    const next = pick(value, browser, res.languages);
    const reload = next.code !== language.code || next.summary !== language.summary;
    if (reload) location.reload();
    return { language: next, reload };
  };
  // 订阅配置（蓝图「界面语言」第 5 条）：别的头、命令行、手改设置文件改了 `ui.language`，当场跟着；掉了队、断了又连上，重新订阅、再读一次
  const followConfig = () => {
    if (!viaCore) return;
    conn.request('subscribe', { stream: 'config' })
      .then(readCore)
      .then((value) => { if (value && value !== setting) applyLanguage(value); })
      .catch((err) => console.error(`订阅不上配置：${err.message}`));
  };
  conn.onPush((method, params) => {
    if (method === 'config.changed') {
      const value = fromConfig(params);
      if (value && value !== setting) applyLanguage(value);
    }
    if (method === 'resync' && params?.stream === 'config') followConfig();
  });
  followConfig();

  const registry = new Registry();
  const slots = new Slots();
  const seams = new Seams();
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
    // 包的样式跟着包挂上、撤下；交回下载完了没有（下载不了的也算完，不卡着包），加载器等它再跑包
    styles: (id, files) => {
      const links = files.map((f) => {
        const link = document.createElement('link');
        link.rel = 'stylesheet';
        link.href = new URL(`packages/${id}/${f}`, base).href;
        link.dataset.package = id;
        document.head.append(link);
        return link;
      });
      const ready = Promise.all(links.map((l) => new Promise((done) => {
        l.addEventListener('load', done, { once: true });
        l.addEventListener('error', done, { once: true });
      })));
      return { ready, remove: () => links.forEach((l) => l.remove()) };
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
        home: info.home,
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
      // 界面语言（蓝图 `web.md`「界面语言」）：现在用哪一种、设置项写的什么、能写哪些、浮层里的几行；改了记进个人那一层，
      // 界面的字、收起那一行的写法有一样变了重新载入页面（界面的字在内核起来时装，包的字在加载时取）
      ctx.provide('language', {
        current: language,
        get setting() { return setting; },
        choices: /** @type {string[]} */ (languageSpec(res.languages).choices),
        /** `/language` 浮层里的几行 */
        options: () => options(setting, browser, res.languages),
        /**
         * 改设置项（`auto` 或表里的一种）；界面的字、时间线收起那一行的写法有一样变了，重新载入页面。交回改完用哪一种、
         * 重不重载。
         */
        set: async (value) => {
          // 核心有配置的写回个人设置（成了才换；拒了的抛出去，由命令那边提示原因）；旧核心只记在这台设备上
          if (viaCore) await conn.request('config.set', { layer: 'personal', changes: [{ key: 'ui.language', value }] });
          return applyLanguage(value);
        },
      });
      ctx.provide('packages', {
        /** 每个包：状态，加上清单里的名字、种类（读不到清单的只有编号） */
        list: async () => Promise.all(loader.status().map(async (s) => ({ ...s, manifest: await manifestOf(s.id).catch(() => null) }))),
        status: () => loader.status(),
        /** 一个包的设置项：清单里的、最终值、每一项来自哪一层（`default`、`distro`、`user`）、写错的（设置页用；停了的包也列） */
        settings: async (id) => {
          const manifest = await manifestOf(id);
          const row = rows(distro.packages, storage.get(USER_LAYER, {})).find((r) => r.id === id);
          const layers = [{ name: 'distro', values: row?.distro ?? {} }, { name: 'user', values: row?.user ?? {} }];
          return { manifest, ...resolve(manifest.settings ?? {}, layers) };
        },
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

/** 连不上桥（蓝图 `web.md`「连核心」第 9 条）：为什么是哪一种，入口照它画一张卡。 */
export class Offline extends Error {
  /** @param {'bad'|'none'|'down'|'core'} kind @param {string} [detail] 桥的原话 */
  constructor(kind, detail = '') {
    super(detail || kind);
    this.kind = kind;
    this.detail = detail;
  }
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
