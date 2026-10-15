#!/usr/bin/env node
'use strict';

const { spawn } = require('node:child_process');
const { ensureBinary } = require('../lib/install');

ensureBinary()
  .then((binary) => {
    const child = spawn(binary, process.argv.slice(2), { stdio: 'inherit' });
    child.on('error', (err) => {
      console.error(`mhtml: failed to start ${binary}: ${err.message}`);
      process.exit(1);
    });
    child.on('exit', (code, signal) => {
      if (signal) process.kill(process.pid, signal);
      else process.exit(code ?? 1);
    });
  })
  .catch((err) => {
    console.error(`mhtml: ${err.message}`);
    process.exit(1);
  });
