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
git -C "$repo_dir" rebase "$base_branch"

git -C "$repo_dir" push origin "$base_branch"
git -C "$repo_dir" push --force-with-lease origin "$pr_integ_branch"

final_rev="$(git -C "$repo_dir" rev-parse HEAD)"

export NEW_REV="$final_rev"

update_cargo_toml() {
  local file="$1"
  local tmp
  tmp="$(mktemp)"
  sed -E 's#(https://github.com/rustls/rustls", rev = ")[0-9a-f]{40}#\1'"$NEW_REV"'#g' "$file" > "$tmp"
  mv "$tmp" "$file"
}

update_cargo_config() {
  local file="$1"
  local tmp
  tmp="$(mktemp)"
  awk -v new_rev="$NEW_REV" '
    BEGIN { in_rustls_block = 0 }
    /^\[source\."git\+https:\/\/github\.com\/rustls\/rustls\?rev=/ {
      gsub(/rev=[0-9a-f]{40}/, "rev=" new_rev)
      in_rustls_block = 1
      print
      next
    }
    in_rustls_block && /^rev = "/ {
      sub(/[0-9a-f]{40}/, new_rev)
      in_rustls_block = 0
    }
    print
  ' "$file" > "$tmp"
  mv "$tmp" "$file"
}

update_cargo_lock() {
  local file="$1"
  local tmp
  tmp="$(mktemp)"
  awk -v new_rev="$NEW_REV" '
    {
      gsub(/git\+https:\/\/github\.com\/rustls\/rustls\?rev=[0-9a-f]{40}#[0-9a-f]{40}/,
           "git+https://github.com/rustls/rustls?rev=" new_rev "#" new_rev)
      print
    }
  ' "$file" > "$tmp"
  mv "$tmp" "$file"
}

update_cargo_toml "$project_dir/Cargo.toml"
update_cargo_config "$project_dir/.cargo/config.toml"
if [ -f "$project_dir/Cargo.lock" ]; then
  update_cargo_lock "$project_dir/Cargo.lock"
fi

echo
git -C "$repo_dir" status --short --branch
echo "origin=$(git -C "$repo_dir" remote get-url origin)"
echo "rev=$final_rev"
