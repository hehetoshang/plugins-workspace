import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const lib = readFileSync(new URL('./src/lib.rs', import.meta.url), 'utf8');
const commands = readFileSync(new URL('./src/commands.rs', import.meta.url), 'utf8');
const desktop = readFileSync(new URL('./src/desktop.rs', import.meta.url), 'utf8');

test('OHOS defines, exports and initializes the standard filesystem backend', () => {
  for (const statement of ['mod desktop;', 'pub use desktop::Fs;', 'app.manage(Fs(app.clone()));']) {
    const escaped = statement.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    assert.match(lib, new RegExp(`#\\[cfg\\(any\\(desktop, target_env = "ohos"\\)\\)\\]\\s*${escaped}`));
  }
});

test('OHOS uses scope-checked path resolution, never the mobile URL shortcut', () => {
  assert.match(commands, /#\[cfg\(any\(desktop, target_env = "ohos"\)\)\]\s*pub fn resolve_file/);
  assert.match(commands, /#\[cfg\(all\(mobile, not\(target_env = "ohos"\)\)\)\]\s*pub fn resolve_file/);
  const standard = commands.slice(commands.indexOf('pub fn resolve_file'), commands.indexOf('fn resolve_file_in_fs'));
  assert.match(standard, /resolve_file_in_fs\(/);
  assert.doesNotMatch(standard, /\.fs\(\)\.open|SafeFilePath::Url/);
  const checked = commands.slice(commands.indexOf('fn resolve_file_in_fs'), commands.indexOf('#[cfg(all(mobile'));
  assert.ok(checked.indexOf('resolve_path(') < checked.indexOf('.open('));
  assert.match(checked, /resolve_path\([\s\S]*?\)\?;/);
});

test('standard backend remains filesystem-only, with no mobile plugin invocation', () => {
  assert.match(desktop, /FilePath::Url\(u\) if u.scheme\(\) == "file"/);
  assert.match(desktop, /FilePath::Url\(_\) => Err/);
  assert.match(desktop, /std::fs::OpenOptions::from\(opts\).open\(path\)/);
  assert.doesNotMatch(desktop, /run_mobile_plugin/);
});
