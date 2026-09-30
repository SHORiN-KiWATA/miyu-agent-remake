// @ts-check
//! 框里的文件块（蓝图 `web.md`「`@` 选文件」第 5 条，照 `tui.md`「输入框」第 12 条的文件块）：`@` 选的文件、目录在框里写成一块
//! `[文件名]`，发出去换回路径写在原来的位置。框本身还是写字的框，块就是框里的一截字，另外记着它对应哪个路径（`Map` 名字 → 路径）；
//! 这里管起名、整块删、换回路径、底下垫的那层字怎么切。纯函数。

/**
 * 块的名字：`[文件名]`，目录 `[名字/]`；超过 `max` 个字的中间截掉写 `…`（留扩展名）；已经有同名、路径不同的，写上一层目录。
 * @param {string} path 写进话里的路径（`model/mention.js` 的 `pathText`）
 * @param {boolean} dir
 * @param {Map<string, string>} taken 已有的块：名字 → 路径
 * @param {number} max
 */
export function blockLabel(path, dir, taken, max) {
  const parts = path.replace(/^'|'$/g, '').replace(/\/$/, '').split('/');
  const make = (n) => {
    let name = parts.slice(-n).join('/');
    if (Array.from(name).length > max) {
      const chars = Array.from(name);
      const dot = name.lastIndexOf('.');
      const ext = dot > 0 && !dir ? Array.from(name.slice(dot)).length : 0;
      const tail = Math.max(ext + 3, Math.floor(max / 3));
      name = `${chars.slice(0, max - tail - 1).join('')}…${chars.slice(-tail).join('')}`;
    }
    return `[${name}${dir ? '/' : ''}]`;
  };
  for (let n = 1; n <= parts.length; n++) {
    const label = make(n);
    if (!taken.has(label) || taken.get(label) === path) return label;
  }
  return make(parts.length);
}

/** 发出去：框里还在的块换回路径（没登记的方括号照原样）。 @param {string} text @param {Map<string, string>} blocks */
export function expand(text, blocks) {
  let out = text;
  for (const [label, path] of blocks) out = out.split(label).join(path);
  return out;
}

/** 框里还在用的块（记进输入历史）。 @param {string} text @param {Map<string, string>} blocks */
export function used(text, blocks) {
  return [...blocks].filter(([label]) => text.includes(label));
}

/**
 * 整块删：`back`（退格）时光标正好在一块后面、`forward`（Delete）时正好在一块前面，删掉整块；别处交 `null`。
 * @param {string} value
 * @param {number} caret
 * @param {string[]} labels
 * @param {'back'|'forward'} dir
 */
export function erase(value, caret, labels, dir) {
  for (const label of labels) {
    const start = dir === 'back' ? caret - label.length : caret;
    if (start >= 0 && value.slice(start, start + label.length) === label) {
      return { value: value.slice(0, start) + value.slice(start + label.length), caret: start };
    }
  }
  return null;
}

/**
 * 底下垫的那层字：块单独成一段（铺底），别的照原样。
 * @param {string} text
 * @param {string[]} labels
 */
export function pieces(text, labels) {
  const out = [];
  let rest = text;
  while (rest) {
    let at = -1;
    let hit = '';
    for (const label of labels) {
      const i = rest.indexOf(label);
      if (i >= 0 && (at < 0 || i < at)) [at, hit] = [i, label];
    }
    if (at < 0) {
      out.push({ text: rest, block: false });
      break;
    }
    if (at > 0) out.push({ text: rest.slice(0, at), block: false });
    out.push({ text: hit, block: true });
    rest = rest.slice(at + hit.length);
  }
  return out;
}
