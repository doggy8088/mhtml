'use strict';

const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { nextVersion, bump } = require('./bump-version');

function project(cargoVersion, npmVersion) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'bump-'));
  fs.mkdirSync(path.join(root, 'npm'));
  fs.writeFileSync(
    path.join(root, 'Cargo.toml'),
    `[package]\nname = "x"\nversion = "${cargoVersion}"\nedition = "2024"\n\n[dependencies]\nfoo = { version = "9.9.9" }\n`
  );
  fs.writeFileSync(path.join(root, 'npm', 'package.json'), `{\n  "name": "x",\n  "version": "${npmVersion}",\n  "engines": {"node": ">=18"}\n}\n`);
  return root;
}

test('computes patch, minor and major bumps', () => {
  assert.strictEqual(nextVersion('0.1.1', 'patch'), '0.1.2');
  assert.strictEqual(nextVersion('0.1.9', 'minor'), '0.2.0');
  assert.strictEqual(nextVersion('0.9.9', 'major'), '1.0.0');
  assert.strictEqual(nextVersion('1.2.3', 'major'), '2.0.0');
  assert.strictEqual(nextVersion('1.2.3', 'minor'), '1.3.0');
});

test('rejects unknown levels and unsupported versions', () => {
  assert.throws(() => nextVersion('1.0.0', 'huge'), /unknown level/);
  assert.throws(() => nextVersion('1.0.0-rc.1', 'patch'), /unsupported version/);
});

test('updates both files and nothing else', () => {
  const root = project('0.1.1', '0.1.1');
  assert.deepStrictEqual(bump(root, 'minor'), { from: '0.1.1', to: '0.2.0' });
  const cargo = fs.readFileSync(path.join(root, 'Cargo.toml'), 'utf8');
  const npm = fs.readFileSync(path.join(root, 'npm', 'package.json'), 'utf8');
  assert.match(cargo, /^version = "0\.2\.0"$/m);
  assert.match(cargo, /foo = \{ version = "9\.9\.9" \}/, 'dependency versions stay untouched');
  assert.strictEqual(JSON.parse(npm).version, '0.2.0');
  assert.match(npm, /"engines"/);
});

test('dry run changes nothing', () => {
  const root = project('0.1.1', '0.1.1');
  assert.strictEqual(bump(root, 'patch', { dryRun: true }).to, '0.1.2');
  assert.match(fs.readFileSync(path.join(root, 'Cargo.toml'), 'utf8'), /version = "0\.1\.1"/);
});

test('refuses when the two versions disagree', () => {
  const root = project('0.1.1', '0.1.0');
  assert.throws(() => bump(root, 'patch'), /versions differ/);
});
