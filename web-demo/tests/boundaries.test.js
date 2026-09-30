// @ts-check
//! 边界（蓝图 `web/architecture.md`「分层」「守着它的」）：软件包只 `import` `src/lib/` 和自己的文件；内核不 `import` 软件包；
//! 每份清单合法：有编号、种类，编号和目录一样，设置项的出厂值过得了自己的校验，`inject` 的服务有人提供（内核的、别的包
//! `provides` 的、职能的名字），发行版里的包都在。整页（`app`）是搬的过程里唯一的例外，拆完就没了。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync, statSync, existsSync } from 'node:fs';
import { join, dirname, resolve, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { problems } from '../src/kernel/config.js';
import { KERNEL_SERVICES } from '../src/kernel/services.js';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const PACKAGES = join(ROOT, 'packages');
/** 搬的过程里还直接用旧代码的包：拆完一个删一个 */
const LEGACY = new Set(['app']);
const SEAMS = ['theme', 'persona', 'markdown', 'highlight', 'math', 'lightbox'];

/** 目录下全部 `.js`。 */
function files(dir) {
  return readdirSync(dir).flatMap((name) => {
    const p = join(dir, name);
    return statSync(p).isDirectory() ? files(p) : p.endsWith('.js') ? [p] : [];
  });
}

/** 一个文件 `import` 了哪些（照相对路径换成绝对路径）。 */
function imports(file) {
  const text = readFileSync(file, 'utf8');
  return [...text.matchAll(/(?:^|\n)\s*(?:import|export)[^'"]*?from\s+'([^']+)'|import\(\s*'([^']+)'\s*\)/g)]
    .map((m) => m[1] ?? m[2]).filter((s) => s.startsWith('.')).map((s) => resolve(dirname(file), s));
}

const ids = readdirSync(PACKAGES).filter((d) => statSync(join(PACKAGES, d)).isDirectory());
const manifests = Object.fromEntries(ids.map((id) => [id, JSON.parse(readFileSync(join(PACKAGES, id, 'manifest.json'), 'utf8'))]));

test('软件包只 import src/lib/ 和自己的文件（整页 app 是搬的过程里的例外）', () => {
  const bad = [];
  for (const id of ids) {
    if (LEGACY.has(id)) continue;
    const own = join(PACKAGES, id);
    for (const file of files(own)) {
      for (const target of imports(file)) {
        if (target.startsWith(own) || target.startsWith(join(ROOT, 'src', 'lib'))) continue;
        bad.push(`${relative(ROOT, file)} → ${relative(ROOT, target)}`);
      }
    }
  }
  assert.deepEqual(bad, []);
});

test('内核、lib 不 import 软件包；lib 只 import lib', () => {
  const bad = [];
  for (const file of files(join(ROOT, 'src'))) {
    for (const target of imports(file)) {
      if (target.startsWith(PACKAGES)) bad.push(`${relative(ROOT, file)} → ${relative(ROOT, target)}`);
      if (file.includes(`${join('src', 'lib')}`) && !target.startsWith(join(ROOT, 'src', 'lib'))) bad.push(`${relative(ROOT, file)} → ${relative(ROOT, target)}`);
    }
  }
  assert.deepEqual(bad, []);
});

test('每份清单合法：编号和目录一样、有种类、设置项的出厂值过得了自己的校验、有入口', () => {
  for (const id of ids) {
    const m = manifests[id];
    assert.equal(m.id, id, `${id} 的清单编号`);
    assert.ok(['base', 'optional'].includes(m.kind), `${id} 的种类`);
    assert.deepEqual(problems(m.settings ?? {}), [], `${id} 的设置项`);
    assert.ok(existsSync(join(PACKAGES, id, 'index.js')), `${id} 的入口`);
    for (const css of m.styles ?? []) assert.ok(existsSync(join(PACKAGES, id, css)), `${id} 的样式 ${css}`);
  }
});

test('inject 的服务都有人提供：内核的、别的包 provides 的、职能', () => {
  const provided = new Set([...KERNEL_SERVICES, ...SEAMS, ...ids.flatMap((id) => manifests[id].provides?.services ?? [])]);
  const bad = ids.flatMap((id) => (manifests[id].inject ?? []).map((n) => n.replace(/[?~]$/, '')).filter((n) => !provided.has(n)).map((n) => `${id} 要 ${n}`));
  assert.deepEqual(bad, []);
});

test('发行版里的包都在', () => {
  const distro = JSON.parse(readFileSync(join(ROOT, 'distro.json'), 'utf8'));
  assert.deepEqual(distro.packages.map((p) => p.id).filter((id) => !ids.includes(id)), []);
});
