#!/bin/bash
# Publish jni-high's crates to crates.io.
#
# Mirrors the pattern used in czkawka/misc/cargo/Publish*.sh: clone a clean
# copy of the repo so no local uncommitted state leaks into the published
# package, then package/publish each crate in dependency order.
#
# Unlike the czkawka scripts, this clones the `master` branch directly rather
# than checking out a version tag: jni-high's history is a single amended +
# force-pushed commit, so a tag would just go stale every time master moves
# and silently publish an old manifest. Push whatever should be published to
# GitHub `master` before running this script.
#
# Order matters: jni-high depends on jni-high-macros with a version
# requirement (not just a path), so jni-high-macros must already be on
# crates.io before jni-high is published, or `cargo publish` for jni-high
# will fail to resolve it.
#
# Usage:
#   ./Publish.sh dry-run     # cargo publish --dry-run for every crate
#   ./Publish.sh publish     # actually publish every crate
#
# NUMBER must match the workspace version in the cloned Cargo.toml (checked
# below) - kept as a hardcoded constant (not a CLI argument) so it can't be
# mistyped when running the real `publish` step. Bump it by hand together
# with `version` in the root Cargo.toml before every release.
#
# `dry-run` on a brand-new, never-before-published set of crates will fail on
# jni-high with "no matching package named jni-high-macros found": this is
# expected, not a bug. `cargo publish --dry-run` never actually uploads
# anything ("aborting upload due to dry run"), so jni-high-macros is still
# absent from crates.io when jni-high's turn comes to resolve its versioned
# dependency. The real `publish` mode works because each crate is actually
# uploaded before the next one needs it.

set -euo pipefail

NUMBER="0.1.0"
MODE="${1:-}"
WORK_PATH="/home/rafal"

if [[ "$MODE" != "dry-run" && "$MODE" != "publish" ]]; then
    echo "Usage: $0 <dry-run|publish>"
    exit 1
fi

cd "$WORK_PATH"
JNI_HIGH_PATH="$WORK_PATH/jni-high-publish-tmp"
rm -rf "$JNI_HIGH_PATH"
git clone --branch master --single-branch https://github.com/qarmin/jni-high.git "$JNI_HIGH_PATH"
cd "$JNI_HIGH_PATH"

CLONED_VERSION=$(grep -m1 '^version = ' Cargo.toml | sed -E 's/version = "(.*)"/\1/')
if [ "$CLONED_VERSION" != "$NUMBER" ]; then
    echo "Version mismatch: Publish.sh has NUMBER=$NUMBER but cloned Cargo.toml has version=$CLONED_VERSION"
    exit 1
fi

publish_crate() {
    local crate_dir="$1"

    cd "$JNI_HIGH_PATH/$crate_dir"
    if ! cargo package; then
        echo "Cargo package failed for $crate_dir"
        exit 1
    fi
    git reset --hard

    cd "$JNI_HIGH_PATH/$crate_dir"
    if [ "$MODE" = "dry-run" ]; then
        publish_ok=0
        cargo publish --dry-run || publish_ok=1
    else
        publish_ok=0
        cargo publish || publish_ok=1
    fi
    if [ "$publish_ok" != 0 ]; then
        echo "Cargo publish failed for $crate_dir"
        exit 1
    fi
    git reset --hard
}

# jni-high-build has no in-workspace dependents, so it can go anywhere in the
# order; jni-high-macros must come before jni-high.
publish_crate "crates/jni-high-macros"
publish_crate "crates/jni-high-build"
publish_crate "crates/jni-high"
