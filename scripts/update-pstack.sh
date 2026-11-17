#!/usr/bin/env bash
# Applies upstream pstack changes to the Amp port in .agents/skills/pstack.
# Usage: scripts/update-pstack.sh [commit]   (default: upstream main)
# Conflicts with our Amp edits are left as conflict markers to resolve by hand.
set -euo pipefail

upstream=https://github.com/cursor/plugins.git
repo_root="$(cd "$(dirname "$0")/.." && pwd)"
dest=.agents/skills/pstack
cd "$repo_root"

base="$(cat "$dest/UPSTREAM_COMMIT")"
git fetch --quiet --depth 1 "$upstream" "$base"
git fetch --quiet --depth 1 "$upstream" "${1:-main}"
target="$(git rev-parse FETCH_HEAD)"

# Paths the port removed, so upstream changes to them are not reapplied.
removed=(
  .cursor-plugin .gitignore plugin.json assets automations docs
  skills/unslop skills/make-bot-ui skills/statechart/scripts/test
)
pathspec=(pstack)
for path in "${removed[@]}"; do pathspec+=(":(exclude)pstack/$path"); done

# git apply cannot delete a file the port edited, so upstream's removals are deleted below.
mapfile -t deleted < <(git diff --name-only --diff-filter=D "$base" "$target" -- "${pathspec[@]}")
stat_pathspec=("${pathspec[@]}")
for path in "${deleted[@]}"; do pathspec+=(":(exclude)$path"); done

git diff --binary "$base" "$target" -- "${pathspec[@]}" > "$repo_root/.pstack-update.patch"
if [ ! -s "$repo_root/.pstack-update.patch" ] && [ ${#deleted[@]} -eq 0 ]; then
  rm "$repo_root/.pstack-update.patch"
  echo "pstack is already at $target"
  exit 0
fi

git diff --stat "$base" "$target" -- "${stat_pathspec[@]}"
status=0
if [ -s "$repo_root/.pstack-update.patch" ]; then
  git apply --3way -p2 --directory="$dest" "$repo_root/.pstack-update.patch" || status=$?
fi
rm "$repo_root/.pstack-update.patch"
# A failed apply changes nothing; conflicts leave markers and still count as applied.
if [ "$status" -ne 0 ] && git diff --quiet -- "$dest"; then
  echo "Upstream changes did not apply; UPSTREAM_COMMIT is unchanged." >&2
  exit "$status"
fi
for path in "${deleted[@]}"; do rm -f "$dest/${path#pstack/}"; done
find "$dest" -type d -empty -delete
echo "$target" > "$dest/UPSTREAM_COMMIT"
echo "Applied upstream pstack $base..$target. Port any new Cursor-specific text in the files above to Amp."
exit "$status"
