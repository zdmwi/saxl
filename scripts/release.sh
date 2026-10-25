#!/bin/sh
# Releases saxl version <X.Y.Z>: bumps every version to it, commits, tags
# v<X.Y.Z>, pushes both, and creates a GitHub release with generated notes.
#
#   scripts/release.sh 0.2.0
#   DRY_RUN=1 scripts/release.sh 0.2.0   # check and bump locally; no commit, tag, or push
#
# Versions follow Semantic Versioning (https://semver.org). Run it from a clean,
# up-to-date main. The Release workflow runs this script; see README "Releases".
set -eu
cd "$(dirname "$0")/.."

die() {
  echo "release: $*" >&2
  exit 1
}

version=${1:-}
[ -n "$version" ] || die "usage: scripts/release.sh <X.Y.Z>"
echo "$version" | grep -Eq '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z.-]+)?$' ||
  die "$version is not a semantic version (X.Y.Z or X.Y.Z-pre, no leading v)"
tag="v$version"

[ "$(git branch --show-current)" = main ] || die "releases are cut from main"
[ -z "$(git status --porcelain --untracked-files=no)" ] || die "the working tree has changes"
git fetch --quiet --tags origin main
[ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] || die "main is not up to date with origin/main"
if git rev-parse --quiet --verify "refs/tags/$tag" >/dev/null; then
  die "$tag already exists"
fi

# The workspace version in Cargo.toml is the source of truth.
current=$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' Cargo.toml)
[ -n "$current" ] || die "can't read the workspace version from Cargo.toml"
# The new version may equal the current one (the first release of a version
# already in Cargo.toml), but never go backwards. Compare X.Y.Z first: `sort -V`
# orders 1.0.0 before 1.0.0-rc.1, but in SemVer a pre-release comes first.
if [ "$version" != "$current" ]; then
  new_core=${version%%-*}
  cur_core=${current%%-*}
  if [ "$new_core" = "$cur_core" ]; then
    # Same X.Y.Z: only a pre-release may be followed by its release or another pre-release.
    [ "$current" != "$cur_core" ] || die "$version is older than the current version $current"
  else
    newest=$(printf '%s\n%s\n' "$cur_core" "$new_core" | sort -V | tail -n 1)
    [ "$newest" = "$new_core" ] || die "$version is older than the current version $current"
  fi

  sed -i.bak '/^\[workspace.package\]/,/^\[/s/^version = ".*"/version = "'"$version"'"/' Cargo.toml
  rm Cargo.toml.bak
  cargo update --workspace --quiet
  # The npm workspaces are private, but keep their versions in step.
  npm version "$version" --workspaces --no-git-tag-version --include-workspace-root=false >/dev/null
fi

if [ -n "${DRY_RUN:-}" ]; then
  echo "release: dry run: would release $tag (current version $current)"
  git status --short
  exit 0
fi

if [ "$version" != "$current" ]; then
  git commit --quiet -am "chore(release): $tag"
fi
git tag -a "$tag" -m "$tag"
git push --quiet origin HEAD:main "refs/tags/$tag"
case $version in
  *-*) gh release create "$tag" --verify-tag --title "$tag" --generate-notes --prerelease ;;
  *) gh release create "$tag" --verify-tag --title "$tag" --generate-notes ;;
esac
echo "release: released $tag"
