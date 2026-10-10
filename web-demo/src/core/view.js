// @ts-check
//! 一个会话手里的条目（蓝图 `web.md`「照条目画」第 1 条；核心 9-8 中、下、补上，`view.md`）：订阅视图流的回应、`view.page` 的一页换上，
//! 之后照推送改：`view.add` 插在 `after` 后面（`null` 是最前）、`view.update` 换掉（带 `after` 的挪位置）、`view.append` 接字
//! （`reply`、`thought` 接 `text`，`tool` 接 `args`）、`view.hidden` 藏起或显示回来、`view.remove` 拿掉、`view.status` 整份换。
//! 往前翻的一页拼在前面，照 `id` 去重。纯的：不碰连接，交回改没改。条目换了就是一个新对象（画的时候照对象比）。

/** 视图流的几种推送 */
export const VIEW_PUSHES = new Set(['view.add', 'view.update', 'view.append', 'view.hidden', 'view.remove', 'view.status']);

export class ViewLog {
  constructor() {
    /** 条目，照显示的先后 @type {any[]} */
    this.list = [];
    /** 会话状态（`view.status`），订阅前是 `null` @type {any} */
    this.status = null;
    /** 还有更早的一页；往前翻从哪一条序号往前（`first`） */
    this.more = false;
    this.first = /** @type {number|null} */ (null);
  }

  /** 订阅回应、最新一页：整份换上。 @param {any} page */
  reset(page) {
    this.list = [...(page?.entries ?? [])];
    if (page && 'status' in page) this.status = page.status ?? null;
    this.more = !!page?.more;
    this.first = page?.first ?? null;
  }

  /** 往前翻的一页：拼在前面，手里已经有的（照 `id`）不重复。 @param {any} page */
  prepend(page) {
    const have = new Set(this.list.map((e) => e.id));
    this.list = [...(page?.entries ?? []).filter((/** @type {any} */ e) => !have.has(e.id)), ...this.list];
    this.more = !!page?.more;
    this.first = page?.first ?? this.first;
  }

  /** @param {string} id */
  at(id) {
    return this.list.findIndex((e) => e.id === id);
  }

  /** 照 `after` 插：`null` 是最前，找不到的接在最后。 @param {any} entry @param {string|null|undefined} after */
  place(entry, after) {
    const i = after == null ? 0 : this.at(after) + 1;
    if (after != null && i === 0) this.list.push(entry);
    else this.list.splice(i, 0, entry);
  }

  /**
   * 一条推送（`method` 是 `view.*`，`params` 是它的参数）。交回改没改。
   * @param {string} method @param {any} params
   */
  apply(method, params) {
    switch (method) {
      case 'view.add': {
        const entry = params.entry;
        const i = this.at(entry.id);
        if (i >= 0) this.list.splice(i, 1);
        this.place(entry, params.after);
        return true;
      }
      case 'view.update': {
        const entry = params.entry;
        const i = this.at(entry.id);
        if (i < 0) {
          this.place(entry, params.after);
          return true;
        }
        if (params.after === undefined) this.list[i] = entry;
        else {
          this.list.splice(i, 1);
          this.place(entry, params.after);
        }
        return true;
      }
      case 'view.append': {
        const i = this.at(params.id);
        if (i < 0) return false;
        const e = this.list[i];
        this.list[i] = e.kind === 'tool' ? { ...e, args: (e.args ?? '') + params.text } : { ...e, text: (e.text ?? '') + params.text };
        return true;
      }
      case 'view.hidden': {
        const ids = new Set(params.ids ?? []);
        let changed = false;
        this.list = this.list.map((e) => {
          if (!ids.has(e.id) || !!e.hidden === !!params.hidden) return e;
          changed = true;
          const { hidden, ...rest } = e;
          return params.hidden ? { ...rest, hidden: true } : rest;
        });
        return changed;
      }
      case 'view.remove': {
        const i = this.at(params.id);
        if (i < 0) return false;
        this.list.splice(i, 1);
        return true;
      }
      case 'view.status':
        this.status = params.status ?? null;
        return true;
      default:
        return false;
    }
  }
}
