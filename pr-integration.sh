#!/usr/bin/env bash

set -euo pipefail

#if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
#  echo "usage: $0 <pr-number> [base-branch]" >&2
#  exit 1
#fi

repo_dir=$(realpath ../src/rustls)
project_dir=$(pwd)
pr_number="${1:-2993}"
base_branch="${2:-main}"
pr_latest_branch="pr-${pr_number}-latest"
pr_integ_branch="pr-${pr_number}-integ"

git -C "$repo_dir" switch "$base_branch"
git -C "$repo_dir" pull --ff-only upstream "$base_branch"

git -C "$repo_dir" fetch upstream "refs/pull/${pr_number}/head"
git -C "$repo_dir" branch -f "$pr_latest_branch" FETCH_HEAD

git -C "$repo_dir" switch -C "$pr_integ_branch" "$pr_latest_branch"
git -C "$repo_dir" rebase --committer-date-is-author-date "$base_branch"

git -C "$repo_dir" push origin "$base_branch"
git -C "$repo_dir" push --force-with-lease origin "$pr_integ_branch"

final_rev="$(git -C "$repo_dir" rev-parse HEAD)"

export NEW_REV="$final_rev"

update_cargo_toml() {
  local file="$1"
  local tmp
  tmp="$(mktemp)"
  while IFS= read -r line || [ -n "$line" ]; do
    case "$line" in
      'rustls = { git = "https://github.com/rustls/rustls", rev = "'*'" }')
        printf 'rustls = { git = "https://github.com/rustls/rustls", rev = "%s" }\n' "$NEW_REV" >> "$tmp"
        ;;
      'rustls-aws-lc-rs = { git = "https://github.com/rustls/rustls", rev = "'*'" }')
        printf 'rustls-aws-lc-rs = { git = "https://github.com/rustls/rustls", rev = "%s" }\n' "$NEW_REV" >> "$tmp"
        ;;
      *)
        printf '%s\n' "$line" >> "$tmp"
        ;;
    esac
  done < "$file"
  mv "$tmp" "$file"
}

update_cargo_config() {
  local file="$1"
  local tmp
  tmp="$(mktemp)"
  local in_rustls_block=0
  while IFS= read -r line || [ -n "$line" ]; do
    case "$line" in
      '[source."git+https://github.com/rustls/rustls?rev='*'"]')
        printf '[source."git+https://github.com/rustls/rustls?rev=%s"]\n' "$NEW_REV" >> "$tmp"
        in_rustls_block=1
        ;;
      'rev = "'*'"')
        if [ "$in_rustls_block" -eq 1 ]; then
          printf 'rev = "%s"\n' "$NEW_REV" >> "$tmp"
          in_rustls_block=0
        else
          printf '%s\n' "$line" >> "$tmp"
        fi
        ;;
      *)
        printf '%s\n' "$line" >> "$tmp"
        ;;
    esac
  done < "$file"
  mv "$tmp" "$file"
}

update_cargo_lock() {
  local file="$1"
  local tmp
  tmp="$(mktemp)"
  while IFS= read -r line || [ -n "$line" ]; do
    case "$line" in
      'source = "git+https://github.com/rustls/rustls?rev='*'#'*'"')
        printf 'source = "git+https://github.com/rustls/rustls?rev=%s#%s"\n' "$NEW_REV" "$NEW_REV" >> "$tmp"
        ;;
      *)
        printf '%s\n' "$line" >> "$tmp"
        ;;
    esac
  done < "$file"
  mv "$tmp" "$file"
}

update_cargo_toml "$project_dir/Cargo.toml"
update_cargo_config "$project_dir/.cargo/config.toml"
if [ -f "$project_dir/Cargo.lock" ]; then
  update_cargo_lock "$project_dir/Cargo.lock"
fi

commit_files=("$project_dir/Cargo.toml" "$project_dir/.cargo/config.toml")
if [ -f "$project_dir/Cargo.lock" ]; then
  commit_files+=("$project_dir/Cargo.lock")
fi

if ! git -C "$project_dir" diff --quiet -- "${commit_files[@]}"; then
  short_rev="${final_rev:0:7}"
  git -C "$project_dir" add -- "${commit_files[@]}"
  git -C "$project_dir" commit -m "sync rustls PR #${pr_number} to ${short_rev}"
fi

echo
git -C "$repo_dir" status --short --branch
echo "origin=$(git -C "$repo_dir" remote get-url origin)"
echo "rev=$final_rev"
