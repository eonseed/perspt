#!/bin/bash

# 🛰️  Perspt PSP Code Check
# Reports and enforces the coding rules Perspt is held to.
#
#   PSP-1  file length     <= 1408 lines
#   PSP-2  function length <= 70 code lines   (Power of Ten Rule 4, relaxed)
#   PSP-3  line width      <= 108 columns
#
# Rust sources only. docs/ — the PSPs and the Sphinx book — is out of scope.
#
# Usage:
#   ./check-rules.sh check                 # gate: fails on any new violation
#   ./check-rules.sh report                # every offending file, function, line
#   ./check-rules.sh report --rule PSP-2  # one rule
#   ./check-rules.sh report --format json  # machine-readable
#   ./check-rules.sh baseline --shrink     # ratchet accepted debt downward
#
# `.cargo/config.toml` is not tracked by this repository, so `cargo xtask` is
# not available from a fresh clone. This script is the portable entry point.
# To get the shorter alias locally, add to your own .cargo/config.toml:
#
#   [alias]
#   xtask = "run --quiet --package xtask --"

set -euo pipefail

cd "$(dirname "$0")"

exec cargo run --quiet --package xtask -- "${@:-report}"
