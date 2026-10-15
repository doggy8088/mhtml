---
name: bump-and-release
description: Bump the version (patch, minor or major), tag it, and release to GitHub Releases and npm.
disable-model-invocation: true
argument-hint: "[patch|minor|major]"
---

# Bump and release

Level (`$LEVEL` below): `$ARGUMENTS` — `patch`, `minor` or `major`; empty means `patch`. Any other value: stop and ask.

A release is public and **irreversible**: npm never lets a version number be reused, and a pushed tag is never moved. Each step ends on a checkpoint; stop at the first red one and report it.

## 1. Preflight

Green means all of:

- branch is `main`: `git rev-parse --abbrev-ref HEAD`
- working tree is clean: `git status --porcelain` prints nothing
- local equals origin: `git fetch origin && git rev-list --left-right --count origin/main...HEAD` prints `0	0`
- Cargo.toml and npm/package.json agree: `make version-check`
- CI is green on this exact commit: `gh run list --branch main --limit 1 --json status,conclusion,headSha` shows `completed`/`success` and `headSha` equals `git rev-parse HEAD`
- `gh auth status` is logged in
- trusted publishing still works: the previous release run succeeded — `gh run list --workflow release.yml --limit 1 --json conclusion -q '.[0].conclusion'` prints `success` (`npm trust list` asks for 2FA, so it cannot serve as a check)

## 2. Plan

```sh
last=$(git describe --tags --abbrev=0)
git log --oneline "$last"..HEAD
node scripts/bump-version.js "$LEVEL" --dry-run     # prints the new version
```

No commits since `$last`: stop, nothing to release. Name any mismatch between level and commits (`feat` under a patch, a breaking change under a minor) and keep the user's level. A `major` from `0.x` means `1.0.0`: say so.

## 3. Confirm

Ask with AskUserQuestion: current → new version, the commit list, and "this publishes to GitHub Releases and npm". Continue only on an explicit yes.

## 4. Bump and verify

```sh
make bump LEVEL="$LEVEL"
make check
```

Done when both pass and `git diff --stat` lists exactly Cargo.toml, Cargo.lock and npm/package.json.

## 5. Commit and tag

Commit with a detailed zh-TW message (the project convention): subject `chore(release): 版本由 X 升為 Y`, body listing the included commits grouped as 新功能 / 修正 / 其他. Then `make tag`, which re-checks everything and creates the annotated tag `vY`.

## 6. Publish

```sh
git push origin main
git push origin vY          # this triggers .github/workflows/release.yml
gh run list --workflow release.yml --limit 1 --json databaseId,headBranch,status
gh run watch <databaseId> --exit-status
```

The run: version check → six builds → GitHub Release → npm publish through OIDC trusted publishing (no token, no OTP). Done when the run concludes `success`.

## 7. Verify

Done when all of:

- `gh release view vY --json assets -q '.assets | length'` prints `12`
- `npm view mhtml-cli@Y version` prints `Y` — the registry lags a minute or more behind the publish, so poll every 15 s for up to 3 min before calling it missing
- `npm view mhtml-cli dist-tags.latest` prints `Y`
- from a scratch directory, `npx -y mhtml-cli@Y --version` prints `mhtml Y`

Report the version, `https://github.com/doggy8088/mhtml/releases/tag/vY` and `https://www.npmjs.com/package/mhtml-cli/v/Y`.

## When something fails

- **Before the GitHub Release job**: nothing is public. Flaky build → `gh run rerun <id> --failed`. Real defect → fix on `main`, then release the next patch.
- **GitHub Release exists, npm job failed**: `vY` is on GitHub only. npm-side cause (trusted publisher missing or not matching `doggy8088/mhtml` + `release.yml`; fix with `npm trust github mhtml-cli --repo doggy8088/mhtml --file release.yml --allow-publish`) → fix, then `gh run rerun <id> --failed`. Workflow defect → fix on `main`, release the next patch.
- **Manual npm publish** needs the user's 2FA, so hand them the command: `! cd npm && cp ../LICENSE ../README.md . && npm publish --access public --otp=<code>`, then `rm LICENSE README.md`.
