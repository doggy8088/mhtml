'use strict';

// Maps Node's platform/arch to the release asset published by the GitHub
// workflow (.github/workflows/release.yml). Keep both in sync.
const TARGETS = {
  'darwin-arm64': 'mhtml-darwin-arm64',
  'darwin-x64': 'mhtml-darwin-x64',
  'linux-arm64': 'mhtml-linux-arm64',
  'linux-x64': 'mhtml-linux-x64',
  'win32-arm64': 'mhtml-win32-arm64.exe',
  'win32-x64': 'mhtml-win32-x64.exe',
};

function assetName(platform = process.platform, arch = process.arch) {
  const name = TARGETS[`${platform}-${arch}`];
  if (!name) {
    throw new Error(
      `mhtml has no prebuilt binary for ${platform}-${arch}. ` +
        'Build it from source instead: https://github.com/doggy8088/mhtml'
    );
  }
  return name;
}

function binaryName(platform = process.platform) {
  return platform === 'win32' ? 'mhtml.exe' : 'mhtml';
}

module.exports = { assetName, binaryName, TARGETS };
