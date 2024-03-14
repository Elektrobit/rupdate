#!/bin/bash -e
# SPDX-License-Identifier: MIT
#
# Script to build and run unit tests
#
# Dependency: ci/build.sh needs to be run before
#
CMDPATH=$(cd "$(dirname "$0")" && pwd)
BASEDIR=${CMDPATH%/*}
UTEST_REPORT="${BASEDIR}/result/utest_report.txt"

# Architecture name amd64, arm64, ...
ARCH=$(dpkg --print-architecture)

RESULTDIR="$BASEDIR/result/$ARCH"

UTEST_REPORT="$RESULTDIR"/utest_report.txt

# Check if ci/build.sh has been run before
if [ ! -d "$RESULTDIR" ]; then
    echo Build environment not set up. Please run ci/build.sh first!
    exit 1
fi

rm -f "$UTEST_REPORT"

# Run tests and copy artifacts
TEST_BINS=$(find "$RESULTDIR"/bin/tests \
    -maxdepth 1 \
    -type f \
    -executable \
    -print
)

set -o pipefail
RETURNCODE=0
cargo test  2>&1 | tee -a "$UTEST_REPORT" || RETURNCODE=$?

exit $RETURNCODE
