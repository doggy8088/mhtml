'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const zlib = require('node:zlib');

const { assetName, binaryName } = require('./platform');

const pkg = require('../package.json');
const VENDOR_DIR = path.join(__dirname, '..', 'vendor');

// MHTML_DOWNLOAD_BASE lets you use a mirror (default: the GitHub release).
function downloadBase() {
  const base =
    process.env.MHTML_DOWNLOAD_BASE ||
    `https://github.com/doggy8088/mhtml/releases/download/v${pkg.version}`;
  return base.replace(/\/+$/, '');
}

function binaryPath() {
  return path.join(VENDOR_DIR, binaryName());
}

async function download(url) {
  const res = await fetch(url, { redirect: 'follow' });
  if (!res.ok) throw new Error(`GET ${url} failed: ${res.status} ${res.statusText}`);
  return Buffer.from(await res.arrayBuffer());
}

/** Downloads, verifies and installs the native binary. Returns its path. */
async function install() {
  const asset = `${assetName()}.gz`;
  const url = `${downloadBase()}/${asset}`;

  const [gz, sumText] = await Promise.all([download(url), download(`${url}.sha256`)]);
  const expected = sumText.toString('utf8').trim().split(/\s+/)[0].toLowerCase();
  const actual = crypto.createHash('sha256').update(gz).digest('hex');
  if (expected !== actual) {
    throw new Error(`checksum mismatch for ${asset}: expected ${expected}, got ${actual}`);
  }

  const target = binaryPath();
  fs.mkdirSync(VENDOR_DIR, { recursive: true });
  const tmp = `${target}.${process.pid}.tmp`;
  fs.writeFileSync(tmp, zlib.gunzipSync(gz), { mode: 0o755 });
  fs.renameSync(tmp, target);
  return target;
}

/** Path of an installed binary, downloading it first when it is missing. */
async function ensureBinary() {
  if (process.env.MHTML_BINARY_PATH) return process.env.MHTML_BINARY_PATH;
  const target = binaryPath();
  if (fs.existsSync(target)) return target;
  console.error(`mhtml: downloading native binary (${assetName()})...`);
  return install();
}

module.exports = { ensureBinary, install, binaryPath, downloadBase };

if (require.main === module) {
  // Runs as the postinstall hook. A failure must not break `npm install`
  // (offline installs, --ignore-scripts mirrors...): bin/mhtml.js retries on
  // first use.
  if (process.env.MHTML_SKIP_DOWNLOAD || process.env.MHTML_BINARY_PATH) process.exit(0);
  install().catch((err) => {
    console.warn(`mhtml: postinstall download skipped (${err.message}). It will be retried on first run.`);
  });
}
