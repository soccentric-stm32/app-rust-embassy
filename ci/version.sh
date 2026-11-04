#!/usr/bin/env bash
#
# version.sh - print the version this build reports, without a leading v
#
# Author:  Sandesh Ghimire <sandesh@soccentric.com>
#
# Used by the Jenkins Package and Release stages.  The source of truth is
# the package version in Cargo.toml; the release tag must be v<this version>.
#

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
sed -n 's/^version *= *"\([^"]*\)".*/\1/p' Cargo.toml | head -n 1
