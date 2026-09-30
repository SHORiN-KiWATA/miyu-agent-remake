// @ts-check
//! 把一个 `list` 挂载位画进一个节点（蓝图 `web/architecture.md`「挂载位」）：挂的东西变了跟着增删、照先后排；每件的画法交回
//! 一个节点，或者 `{el, dispose}`（拿下来时调 `dispose`）；画的时候抛错的，那一格换成 `failed` 画的「这一块出错了」，别的照常。

/**
 * @typedef {{list: (name: string) => any[], watch: (name: string, fn: () => void) => () => void,
 *   draw: (entry: any, ...args: any[]) => any}} SlotsApi 按包绑的一份挂载位（`ctx.slots`）
 */

/**
 * @param {HTMLElement} host 画在哪
 * @param {SlotsApi} slots
 * @param {string} name 挂载位
 * @param {(owner: string, reason: string) => Node} failed 抛错的那一格画什么
 * @returns {() => void} 不画了：拿下画的，调每件的 `dispose`
 */
export function mountList(host, slots, name, failed) {
  /** @type {Map<any, {el: Node, dispose?: () => void}>} */
  const drawn = new Map();
  const drop = (entry, d) => {
    try {
      d.dispose?.();
    } catch (err) {
      console.error(`${entry.owner} 挂的 ${entry.id} 拿下来时出错了`, err);
    }
    d.el.parentNode?.removeChild(d.el);
    drawn.delete(entry);
  };
  const sync = () => {
    const entries = slots.list(name);
    for (const [entry, d] of [...drawn]) if (!entries.includes(entry)) drop(entry, d);
    /** @type {Node|null} */
    let prev = null;
    for (const entry of entries) {
      let d = drawn.get(entry);
      if (!d) {
        const out = slots.draw(entry);
        d = out instanceof Node ? { el: out } : out?.failed ? { el: failed(out.owner, out.reason) } : out;
        if (!d?.el) d = { el: failed(entry.owner, '画法没有交回节点') };
        drawn.set(entry, d);
      }
      const want = prev ? prev.nextSibling : host.firstChild;
      if (want !== d.el) host.insertBefore(d.el, want);
      prev = d.el;
    }
  };
  const stop = slots.watch(name, sync);
  sync();
  return () => {
    stop();
    for (const [entry, d] of [...drawn]) drop(entry, d);
  };
}
