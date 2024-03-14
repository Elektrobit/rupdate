#!/bin/bash -e
# SPDX-License-Identifier: MIT
#
# script to prepare and run cargo check on the project
#
# Dependency: ci/build.sh needs to be run before
#
CMDPATH=$(cd "$(dirname "$0")" && pwd)
BASEDIR=${CMDPATH%/*}

# Architecture name amd64, arm64, ...
ARCH=$(dpkg --print-architecture)

# Architecture name x86_64, aarch64, ...
ARCH_ALT=$(uname -m)

TARGET_TRIPLET=${ARCH_ALT}-unknown-linux-gnu
RESULTDIR="$BASEDIR/result/$ARCH"

rm -rf "${RESULTDIR}/rust-lint"
mkdir -p "${RESULTDIR}/rust-lint"

cd "${BASEDIR}"

# Catch errors even though we use a pipe to tee
set -o pipefail
cargo clippy --workspace --target "${TARGET_TRIPLET}" \
    | tee "${RESULTDIR}/rust-lint/report-rupdate"
