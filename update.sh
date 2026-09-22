#!/usr/bin/env bash
set -euo pipefail

fail() { printf 'Update stopped: %s\n' "$*" >&2; exit 1; }
command -v git >/dev/null 2>&1 || fail 'Git is required. Install it and try again.'
cd -- "$(dirname -- "${BASH_SOURCE[0]}")"

[[ -e .git ]] || fail 'This folder is not a Git checkout. Clone https://github.com/Splinters2006/splinterparty.git first.'
[[ "$(git symbolic-ref --quiet --short HEAD || true)" == main ]] || fail 'Switch to the main branch before updating.'
[[ -z "$(git status --porcelain --untracked-files=all)" ]] || fail 'Local changes or untracked files exist. Commit, stash, or move them before updating; nothing was overwritten.'
git remote get-url origin >/dev/null 2>&1 || fail 'No origin remote is configured.'

printf 'Checking for splinterparty updates…\n'
git fetch origin main
git merge-base --is-ancestor HEAD FETCH_HEAD || fail 'Local commits differ from the remote. Resolve them manually before updating.'
before=$(git rev-parse HEAD)
git merge --ff-only FETCH_HEAD
after=$(git rev-parse HEAD)

if [[ "$before" == "$after" ]]; then
  printf 'splinterparty is already up to date (%s).\n' "${after:0:7}"
else
  printf 'Updated splinterparty: %s → %s.\n' "${before:0:7}" "${after:0:7}"
fi
printf 'No dependency installation or build is needed.\n'
printf 'Restart your running server to load the updated code (use your usual PORT setting).\n'
printf 'Restarting clears current rooms and uploads.\n'
