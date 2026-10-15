#!/usr/bin/env node
'use strict';

// Bumps the version in Cargo.toml and npm/package.json together.
//
//   node scripts/bump-version.js <patch|minor|major> [--dry-run] [--root <dir>]
//
// Prints the new version on stdout. Both files must currently agree, and only
// plain X.Y.Z versions are handled (no pre-release suffixes).

const fs = require('node:fs');
const path = require('node:path');

const LEVELS = ['patch', 'minor', 'major'];
const SEMVER = /^(\d+)\.(\d+)\.(\d+)$/;

function nextVersion(current, level) {
  const m = SEMVER.exec(current);
  if (!m) throw new Error(`unsupported version "${current}" (expected X.Y.Z)`);
  let [major, minor, patch] = m.slice(1).map(Number);
  if (level === 'major') [major, minor, patch] = [major + 1, 0, 0];
  else if (level === 'minor') [minor, patch] = [minor + 1, 0];
  else if (level === 'patch') patch += 1;
  else throw new Error(`unknown level "${level}" (use ${LEVELS.join(', ')})`);
  return `${major}.${minor}.${patch}`;
}

const CARGO_RE = /^version = "([^"]+)"/m;
const NPM_RE = /"version":\s*"([^"]+)"/;

function readVersions(root) {
  const cargo = fs.readFileSync(path.join(root, 'Cargo.toml'), 'utf8');
  const npm = fs.readFileSync(path.join(root, 'npm', 'package.json'), 'utf8');
  return { cargo, npm, cargoVersion: CARGO_RE.exec(cargo)?.[1], npmVersion: NPM_RE.exec(npm)?.[1] };
}

function bump(root, level, { dryRun = false } = {}) {
  const { cargo, npm, cargoVersion, npmVersion } = readVersions(root);
  if (!cargoVersion || !npmVersion) throw new Error('could not find the version in Cargo.toml or npm/package.json');
  if (cargoVersion !== npmVersion) {
    throw new Error(`versions differ: Cargo.toml=${cargoVersion} npm/package.json=${npmVersion}; align them first`);
  }
  const next = nextVersion(cargoVersion, level);
  if (!dryRun) {
    fs.writeFileSync(path.join(root, 'Cargo.toml'), cargo.replace(CARGO_RE, `version = "${next}"`));
    fs.writeFileSync(path.join(root, 'npm', 'package.json'), npm.replace(NPM_RE, `"version": "${next}"`));
  }
  return { from: cargoVersion, to: next };
}

module.exports = { nextVersion, bump };

if (require.main === module) {
  const args = process.argv.slice(2);
  const rootIdx = args.indexOf('--root');
  const root = rootIdx >= 0 ? args[rootIdx + 1] : path.join(__dirname, '..');
  const dryRun = args.includes('--dry-run');
  const level = args.find((a, i) => !a.startsWith('--') && args[i - 1] !== '--root');
  try {
    if (!level) throw new Error(`usage: bump-version.js <${LEVELS.join('|')}> [--dry-run]`);
    const { from, to } = bump(root, level, { dryRun });
    console.error(`${dryRun ? '[dry run] ' : ''}${from} -> ${to}`);
    console.log(to);
  } catch (err) {
    console.error(`bump-version: ${err.message}`);
    process.exit(1);
  }
}
