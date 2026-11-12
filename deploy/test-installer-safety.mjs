import assert from 'node:assert/strict';
import fs from 'node:fs';

const template = fs.readFileSync('src-tauri/windows/nsis/installer.nsi', 'utf8');
const hooks = fs.readFileSync('src-tauri/windows/nsis/installer-hooks.nsi', 'utf8');
assert.match(template, /^!include "Win\\RestartManager\.nsh"$/m,
  'The custom template must include the Restart Manager macros used by the pinned Tauri CLI');
const uninstall = template.split('Section Uninstall')[1]?.split('SectionEnd')[0];
assert.ok(uninstall, 'Uninstaller section must exist');
const check = uninstall.indexOf('!insertmacro CheckIfAppIsRunning');
const hook = uninstall.indexOf('!insertmacro NSIS_HOOK_PREUNINSTALL');
const deletion = uninstall.indexOf('Delete ');
assert.ok(check >= 0 && check < hook && check < deletion,
  'The running-app/cancel check must precede hooks and deletion');
assert.doesNotMatch(uninstall + hooks, /^\s*RMDir\s+\/r\b/im,
  'Uninstall must never recursively remove directories');
assert.doesNotMatch(uninstall + hooks, /^\s*Delete\s+[^\r\n]*\*/im,
  'Uninstall must never delete unknown files with wildcards');
assert.doesNotMatch(template + hooks, /DeleteAppDataCheckbox|RemoveRuntimePathIfSafe/,
  'Uninstall must not offer deletion of user-selected data directories');
assert.doesNotMatch(hooks, /^\s*(?:FindFirst|FindNext|Delete|RMDir)\b/im,
  'Hooks must not enumerate or delete user files');
console.log('Installer policy checks passed: cancellation precedes deletion; user data is retained.');
