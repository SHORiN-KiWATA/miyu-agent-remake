// @ts-check
//! 编进内核的服务的名字（蓝图 `web/architecture.md`「分层」）：`boot.js` 照它提供，边界的检查照它认哪些服务一定在。

export const KERNEL_SERVICES = ['core', 'sessions', 'host', 'page', 'slots', 'seams', 'storage', 'packages', 'language'];
