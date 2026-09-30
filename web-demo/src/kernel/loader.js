// @ts-check
//! 加载器（蓝图 `web/architecture.md`「发行版」，照 Cordis 的 Loader）：发行版（`distro.json`）和个人那一层合成包的列表，照
//! `id` 对账：新的加载，没了的撤回，停用的撤回，配置变了重装（撤回照登记的反序，重装干干净净）。每个包一个纤程
//! （`context.js`）；它的样式跟着它挂上、撤下。清单自己有毛病（出厂值过不了校验）、找不到的，只它故障。

import { Fiber } from './context.js';
import { resolve, problems } from './config.js';

/**
 * @typedef {{id: string, disabled?: boolean, config?: Record<string, any>}} DistroRow 发行版里的一行
 * @typedef {{disabled?: boolean, config?: Record<string, any>}} UserPatch 个人那一层给一个包的补丁
 * @typedef {{id: string, disabled: boolean, distro: Record<string, any>, user: Record<string, any>}} Row 合好的一行
 * @typedef {{load: (id: string) => Promise<import('./context.js').Package>,
 *   styles: (id: string, files: string[]) => () => void}} Drivers 怎么拿到一个包（清单加入口）、怎么挂它的样式
 */

/**
 * 发行版和个人那一层合成包的列表：个人按 `id` 停用、启用、改配置；发行版里没有的包不加（装新包以后再说）。
 * @param {DistroRow[]} distro
 * @param {Record<string, UserPatch>} user
 * @returns {Row[]}
 */
export function rows(distro, user) {
  return distro.map((d) => ({
    id: d.id,
    disabled: user[d.id]?.disabled ?? d.disabled ?? false,
    distro: d.config ?? {},
    user: user[d.id]?.config ?? {},
  }));
}

export class Loader {
  /**
   * @param {import('./context.js').Registry} registry
   * @param {Drivers} drivers
   */
  constructor(registry, drivers) {
    this.registry = registry;
    this.drivers = drivers;
    /** @type {Map<string, {sig: string, fiber: Fiber|null, state?: string, reason?: string, errors: any[], settings?: Record<string, any>, values?: Record<string, any>}>} */
    this.loaded = new Map();
    /** @type {Row[]} */
    this.current = [];
  }

  /**
   * 照新的列表对账。一个个来：先撤回该撤的，再按列表的先后加载（等服务的自己等着，来了就起）。
   * @param {Row[]} next
   */
  async sync(next) {
    this.current = next;
    const want = new Map(next.map((r) => [r.id, r]));
    for (const [id, had] of [...this.loaded]) {
      const row = want.get(id);
      if (row && !row.disabled && JSON.stringify(row) === had.sig) continue;
      if (row && !row.disabled && this.live(had, row)) continue;
      had.fiber?.dispose();
      this.loaded.delete(id);
    }
    for (const row of next) {
      if (row.disabled || this.loaded.has(row.id)) continue;
      await this.start(row);
    }
  }

  /**
   * 只改了 `applies: live` 的设置项：当场换给这个包（`Fiber.update`），不重装；交回换没换成。
   * @param {any} had 加载着的那一份
   * @param {Row} row 新的一行
   */
  live(had, row) {
    if (!had.fiber || !had.settings || had.fiber.state !== 'active') return false;
    const config = resolve(had.settings, [{ name: 'distro', values: row.distro }, { name: 'user', values: row.user }]);
    const changed = Object.keys(had.settings).filter((k) => JSON.stringify(config.values[k]) !== JSON.stringify(had.values?.[k]));
    if (!changed.every((k) => had.settings?.[k].applies === 'live')) return false;
    had.sig = JSON.stringify(row);
    had.values = config.values;
    had.errors = config.errors;
    if (changed.length) had.fiber.update(config.values);
    return true;
  }

  /** 加载一个：拿清单和入口，合配置，起纤程；它的样式包进纤程，跟着它挂上、撤下。 */
  async start(row) {
    const sig = JSON.stringify(row);
    let pkg;
    try {
      pkg = await this.drivers.load(row.id);
    } catch (err) {
      this.loaded.set(row.id, { sig, fiber: null, state: 'failed', reason: err instanceof Error ? err.message : String(err), errors: [] });
      return;
    }
    const settings = pkg.manifest.settings ?? {};
    const bad = problems(settings);
    if (bad.length) {
      this.loaded.set(row.id, { sig, fiber: null, state: 'failed', reason: `清单的设置项有毛病：${bad.map((b) => `${b.key}（${b.error}）`).join('；')}`, errors: [] });
      return;
    }
    const config = resolve(settings, [{ name: 'distro', values: row.distro }, { name: 'user', values: row.user }]);
    const styles = pkg.manifest.styles ?? [];
    const fiber = new Fiber(this.registry, {
      manifest: pkg.manifest,
      apply: (ctx) => {
        if (styles.length) ctx.effect(() => this.drivers.styles(row.id, styles));
        pkg.apply(ctx);
      },
    }, config.values);
    this.loaded.set(row.id, { sig, fiber, errors: config.errors, settings, values: config.values });
    fiber.start();
  }

  /** 每个包在哪一步：就绪、等着（缺什么）、故障（为什么）、停了；个人那一层写错了的项。 */
  status() {
    return this.current.map((row) => {
      if (row.disabled) return { id: row.id, state: 'disabled', missing: [], reason: null, errors: [] };
      const had = this.loaded.get(row.id);
      if (!had) return { id: row.id, state: 'idle', missing: [], reason: null, errors: [] };
      const fiber = had.fiber;
      return {
        id: row.id,
        state: fiber ? fiber.state : had.state,
        missing: fiber ? fiber.missing() : [],
        reason: fiber ? fiber.reason : had.reason,
        errors: had.errors,
      };
    });
  }
}
