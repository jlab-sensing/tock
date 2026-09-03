#!/usr/bin/env bash

# Licensed under the Apache License, Version 2.0 or the MIT License.
# SPDX-License-Identifier: Apache-2.0 OR MIT
# Copyright Tock Contributors 2026.
#
# Build-check every optional feature configuration for the LoRa E5 Mini board.
#
# Usage (from this board directory):
#   ./test-feature-configs.sh
#   make test-features
#
# Optional environment:
#   MAKE_TARGET   make goal to run for each config (default: check)
#   FAIL_ON_WARN  if set to 1, treat compiler warnings as failures

set -euo pipefail

BOARD_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${BOARD_DIR}"

MAKE_TARGET="${MAKE_TARGET:-check}"
FAIL_ON_WARN="${FAIL_ON_WARN:-0}"

# Each entry is: display_label|make_args...
# Empty make_args means the default (no optional features).
CONFIGS=(
  "default (no flags)|"
  "process-console|process-console=1"
  "debug-macro|debug-macro=1"
  "halt-on-panic|halt-on-panic=1"
  "process-console + debug-macro|process-console=1 debug-macro=1"
  "process-console + halt-on-panic|process-console=1 halt-on-panic=1"
  "debug-macro + halt-on-panic|debug-macro=1 halt-on-panic=1"
  "all three|process-console=1 debug-macro=1 halt-on-panic=1"
  "dev=1|dev=1"
  "--process-console|-- --process-console"
  "--debug-macro|-- --debug-macro"
  "--halt-on-panic|-- --halt-on-panic"
  "--dev|-- --dev"
  "--process-console --debug-macro --halt-on-panic|-- --process-console --debug-macro --halt-on-panic"
)

pass=0
fail=0
failures=()

printf 'Testing %s feature configurations (make %s)\n\n' \
  "${#CONFIGS[@]}" "${MAKE_TARGET}"

for entry in "${CONFIGS[@]}"; do
  label="${entry%%|*}"
  args="${entry#*|}"

  # shellcheck disable=SC2206
  make_args=(${args})

  printf '%-55s ' "${label}"

  set +e
  if [[ ${#make_args[@]} -eq 0 || -z "${args}" ]]; then
    out="$(make "${MAKE_TARGET}" 2>&1)"
  else
    out="$(make "${MAKE_TARGET}" "${make_args[@]}" 2>&1)"
  fi
  status=$?
  set -e

  warn_count="$(printf '%s\n' "${out}" | grep -c '^warning:' || true)"

  if [[ ${status} -ne 0 ]]; then
    printf 'FAIL (exit %s)\n' "${status}"
    failures+=("${label}: build failed")
    fail=$((fail + 1))
    printf '%s\n' "${out}" | grep -E 'error(\[|:)|Error' | head -20 || true
    continue
  fi

  if [[ "${FAIL_ON_WARN}" == "1" && "${warn_count}" -gt 0 ]]; then
    printf 'FAIL (warnings=%s)\n' "${warn_count}"
    failures+=("${label}: ${warn_count} warning(s)")
    fail=$((fail + 1))
    printf '%s\n' "${out}" | grep -E '^warning:|--> ' | head -40 || true
    continue
  fi

  printf 'PASS (warnings=%s)\n' "${warn_count}"
  pass=$((pass + 1))
done

printf '\nRESULT: %s passed, %s failed\n' "${pass}" "${fail}"

if [[ ${fail} -ne 0 ]]; then
  printf '\nFailures:\n'
  for f in "${failures[@]}"; do
    printf '  - %s\n' "${f}"
  done
  exit 1
fi
