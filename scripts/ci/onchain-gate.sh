#!/usr/bin/env bash
#
# nightly.yml `multisig-fork`: are the two services the fork harness runs
# (scripts/onchain/multisig-fork/up.sh) checked out at a pinned commit?
#
#   GENESIS_REF          the safe-genesis-registry commit asked for (may be empty)
#   GENESIS_OUTCOME      outcome of its checkout step ('' when it was not tried)
#   PEERCONNECT_REF      the peerconnect commit asked for
#   PEERCONNECT_OUTCOME  outcome of its checkout step
#
# Writes ready=true|false to $GITHUB_OUTPUT. When not ready it says EXACTLY
# what is missing — in the log, as a workflow warning, and in the job summary —
# and exits 0: a missing service is a configuration the owner has to provide,
# not a regression in the wallet, and the job must not read as one.
set -euo pipefail

out() { echo "$1" >>"${GITHUB_OUTPUT:-/dev/null}"; }
summary() { echo "$1" >>"${GITHUB_STEP_SUMMARY:-/dev/null}"; }

why=""
if [[ -z "${GENESIS_REF:-}" ]]; then
  why="No genesis service commit is pinned. Set the repository variable SGR_MAILBOX_REF (Settings → Secrets and variables → Actions → Variables) to the commit of mondaylabsltd/safe-genesis-registry whose genesis-server carries the proposal mailbox the wallet speaks to — or pass -f genesis_ref=<sha> to a manual run. The build used locally (/Volumes/data/production3/sgr-mailbox, branch mailbox-any-signer) is not on GitHub yet: push that branch first, then pin its commit."
elif [[ "${GENESIS_OUTCOME:-}" != success ]]; then
  why="mondaylabsltd/safe-genesis-registry@${GENESIS_REF} could not be checked out (${GENESIS_OUTCOME:-not tried}). Check that the commit is pushed, and — if the repository is private — that the ONCHAIN_REPOS_TOKEN secret is a token with read access to it."
elif [[ "${PEERCONNECT_OUTCOME:-}" != success ]]; then
  why="atshelchin/peerconnect@${PEERCONNECT_REF:-?} could not be checked out (${PEERCONNECT_OUTCOME:-not tried}). Set the repository variable PEERCONNECT_REF to a pushed commit, and — if the repository is private — ONCHAIN_REPOS_TOKEN to a token that can read it."
fi

if [[ -n "$why" ]]; then
  echo "SKIPPED: the multisig fork suite did not run."
  echo "$why"
  echo "::warning title=multisig fork suite skipped::$why"
  summary "### Multisig fork suite: SKIPPED"
  summary ""
  summary "$why"
  out "ready=false"
  exit 0
fi

echo "genesis service: safe-genesis-registry@${GENESIS_REF}; peerconnect@${PEERCONNECT_REF}"
summary "### Multisig fork suite"
summary ""
summary "genesis service \`safe-genesis-registry@${GENESIS_REF}\`, relay \`peerconnect@${PEERCONNECT_REF}\`."
out "ready=true"
