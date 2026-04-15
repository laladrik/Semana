#!/bin/bash --norc
# The scripts takes a GDB script and a package
# Usage:
# ./gdbtest.sh .gdb/debug_leap_years.gdb calendar

set -e

GDBSCRIPT="$1"
cmd=("rust-gdb")
if test -z "$GDBSCRIPT"; then
    echo "Usage: gdbtest.sh <gdb-script> <package>"
    exit 1;
fi

cmd+=("-x" "$GDBSCRIPT")
PACKAGENAME="$2"

if test -z "$PACKAGENAME"; then
    echo "Usage: gdbtest.sh <gdb-script> <package>"
    exit 1;
fi

executable=$(cargo test --no-run --all --message-format=json-render-diagnostics \
    | jq -sr '.[] | select(.executable != null) | .executable' \
    | grep "$PACKAGENAME")


if test -z "$executable"; then
    echo "the package "'"'"$PACKAGENAME"'"'" does not have an executable."
    exit 1;
fi

cmd+=("$executable")
"${cmd[@]}"
