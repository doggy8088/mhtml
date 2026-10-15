'use strict';

const test = require('node:test');
const assert = require('node:assert');
const { assetName, binaryName, TARGETS } = require('../lib/platform');

test('maps supported platforms to release assets', () => {
  assert.strictEqual(assetName('darwin', 'arm64'), 'mhtml-darwin-arm64');
  assert.strictEqual(assetName('linux', 'x64'), 'mhtml-linux-x64');
  assert.strictEqual(assetName('win32', 'x64'), 'mhtml-win32-x64.exe');
});

test('rejects unsupported platforms with a helpful message', () => {
  assert.throws(() => assetName('freebsd', 'x64'), /no prebuilt binary for freebsd-x64/);
});

test('windows binaries get an .exe suffix', () => {
  assert.strictEqual(binaryName('win32'), 'mhtml.exe');
  assert.strictEqual(binaryName('linux'), 'mhtml');
  for (const [key, asset] of Object.entries(TARGETS)) {
    assert.strictEqual(asset.endsWith('.exe'), key.startsWith('win32'));
  }
});
