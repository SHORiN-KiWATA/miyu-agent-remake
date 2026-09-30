// @ts-check
//! 回答里写的路径换成本机的绝对路径（蓝图 `web.md`「读本机文件」）：绝对路径、`~/` 开头、`file://` 开头的照写；别的相对路径照
//! 这个会话的工作目录找；网上的地址（有协议的）不是本机路径。纯函数，桥的 `/file` 照它取。

/**
 * @param {string} target 她写的地址
 * @param {{home: string|null, cwd: string|null}} where 家目录（`web.info`）、这个会话的工作目录（`session.created`）
 * @returns {string|null} 绝对路径；不是本机路径的是 `null`
 */
export function localPath(target, where) {
  const raw = (target ?? '').trim();
  if (!raw) return null;
  if (/^file:\/\//i.test(raw)) {
    try { return normalize(decodeURIComponent(raw.replace(/^file:\/\//i, ''))); } catch { return null; }
  }
  if (/^[a-z][a-z0-9+.-]*:/i.test(raw)) return null;
  if (raw.startsWith('~/')) return where.home ? normalize(`${where.home}/${raw.slice(2)}`) : null;
  if (raw.startsWith('/')) return normalize(raw);
  return where.cwd ? normalize(`${where.cwd}/${raw}`) : null;
}

/** 去掉 `.`、`..`、重复的 `/`；越过根的 `..` 停在根上。 */
function normalize(path) {
  const out = [];
  for (const part of path.split('/')) {
    if (!part || part === '.') continue;
    if (part === '..') out.pop();
    else out.push(part);
  }
  return `/${out.join('/')}`;
}
