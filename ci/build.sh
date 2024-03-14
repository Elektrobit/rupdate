#!/bin/bash -e
# SPDX-License-Identifier: MIT
#
# project build script
#
CMDPATH=$(cd "$(dirname "${0}")" && pwd)
BASEDIR=${CMDPATH%/*}

# architecture name amd64, arm64, ...
ARCH=$(dpkg --print-architecture)

# architecture name x86_64, aarch64, ...
ARCH_ALT=$(uname -m)

TARGET_TRIPLET=${ARCH_ALT}-unknown-linux-gnu

BUILD_TYPE="release"
if [ -n "$1" ]; then
    BUILD_TYPE="$1"
    shift
fi

BUILDDIR="$BASEDIR/target/$TARGET_TRIPLET"
if [ "$BUILD_TYPE" = "release" ]; then
    RESULTDIR="$BASEDIR/result/$ARCH"
else
    RESULTDIR="$BASEDIR/result/$ARCH-$BUILD_TYPE"
fi

# prepare result dir
rm -rf "${RESULTDIR}"
mkdir -p "${RESULTDIR}"/{bin/tests,doc}

cd "${BASEDIR}"

# Log dependency tree
cargo tree

# Build and copy binaries
if [ "$BUILD_TYPE" = "release" ]; then
     cargo build --target "${TARGET_TRIPLET}" --release
else
     cargo build --target "${TARGET_TRIPLET}"
fi

find "${BUILDDIR}/${BUILD_TYPE}" \
    -maxdepth 1 \
    -type f \
    -executable \
    -exec cp {} "${RESULTDIR}"/bin \;

# Build tests
cargo build --target "${TARGET_TRIPLET}" --tests
find "${BUILDDIR}"/debug/deps/ \
    -maxdepth 1 \
    -type f \
    -executable \
    -exec cp {} "${RESULTDIR}"/bin/tests/ \;

# Build docs
cargo doc --release --no-deps --target "${TARGET_TRIPLET}"
cp -r "${BUILDDIR}"/doc/* "${RESULTDIR}"/doc/

# Create the manual
"${BASEDIR}/scripts/manual/update-tool-gen-manual"
mv "${BASEDIR}/manual.txt" "${RESULTDIR}/"
