import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';

const directory = process.argv[2];
if (!directory) throw new Error('Usage: node deploy/fork-release-manifest.mjs <directory>');
const config = JSON.parse(fs.readFileSync('src-tauri/tauri.conf.json', 'utf8'));
const installers = fs.readdirSync(directory).filter(name => name.endsWith('-setup.exe'));
if (installers.length !== 1) throw new Error('Expected exactly one x64 installer');
const installer = installers[0];
const signature = fs.readFileSync(path.join(directory, `${installer}.sig`), 'utf8').trim();
if (!signature) throw new Error('Missing update signature');
const repository = 'Adularia-Z/mz-browser';
const manifest = {
  version: config.version,
  notes: 'Adularia fork: preserve user files and browser data on uninstall; reject plaintext secret fallback.',
  pub_date: new Date().toISOString(),
  platforms: {
    'windows-x86_64': {
      signature,
      url: `https://github.com/${repository}/releases/download/v${config.version}/${encodeURIComponent(installer)}`,
    },
  },
};
fs.writeFileSync(path.join(directory, 'latest.json'), JSON.stringify(manifest, null, 2) + '\n');
fs.writeFileSync(path.join(directory, 'build-provenance.json'), JSON.stringify({
  repository,
  commit: process.env.GITHUB_SHA,
  run: process.env.GITHUB_RUN_ID,
  version: config.version,
  target: 'x86_64-pc-windows-msvc',
  installerPolicy: 'Uninstall preserves browser data and unrelated files.',
}, null, 2) + '\n');
const sums = fs.readdirSync(directory).sort().map(name => {
  const hash = crypto.createHash('sha256').update(fs.readFileSync(path.join(directory, name))).digest('hex');
  return `${hash}  ${name}`;
});
fs.writeFileSync(path.join(directory, 'SHA256SUMS.txt'), sums.join('\n') + '\n');
