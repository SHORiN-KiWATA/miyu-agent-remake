// @ts-check
//! 这台设备上存的东西（蓝图 `web/architecture.md`「多用户、多终端」第 5 条）：存法是宿主给的（浏览器是 `localStorage`），
//! 键带上握手回的账号，同一台电脑上换一个人登录看不到上一个人的。值存成 JSON；读写抛错（隐私窗口、清过数据）、存的读不懂的，
//! 照没有算，页面照样起得来。

/**
 * @param {import('../host/browser.js').Store} store 宿主给的存法
 * @param {string} account 这个页面登录成的账号
 * @param {Record<string, string>} [legacy] 原来不分账号的旧键：`{新名字: 旧键}`，第一次读时搬到这个账号名下
 */
export function accountStorage(store, account, legacy = {}) {
  const full = (/** @type {string} */ key) => `miyu.${account}.${key}`;
  /** 读一个键的原样；旧键有、新键没有的先搬过来 */
  const raw = (/** @type {string} */ key) => {
    const got = store.getItem(full(key));
    const old = legacy[key];
    if (got != null || !old) return got;
    const moved = store.getItem(old);
    if (moved != null) {
      store.setItem(full(key), moved);
      store.removeItem(old);
    }
    return moved;
  };
  return {
    /** 读；没有的、读不了的交 `fallback`。 */
    get(/** @type {string} */ key, /** @type {any} */ fallback) {
      try {
        const v = raw(key);
        return v == null ? fallback : JSON.parse(v);
      } catch {
        return fallback;
      }
    },
    /** 写；写不了的只记一条（这一次照样用）。 */
    set(/** @type {string} */ key, /** @type {any} */ value) {
      try {
        store.setItem(full(key), JSON.stringify(value));
      } catch (err) {
        console.error(`存不下 ${key}`, err);
      }
    },
  };
}
