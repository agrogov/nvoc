#!/usr/bin/env bash
# Diff one RM control between two rmtrace logs, with the noise removed.
#
#   ./rmdiff.sh A.log B.log 0x20802096     # one command
#   ./rmdiff.sh A.log B.log                # every command present in both
#
# Raw logs differ on every line: timestamps, sequence numbers and kernel-assigned handles change per run. This keeps
# only each call's command, size, status and parameter words, numbering repeated calls to the same command, so what is
# left in the diff is the bytes that actually moved.
set -euo pipefail

[[ $# -ge 2 ]] || { echo "usage: $0 A.log B.log [cmd]" >&2; exit 2; }
a=$1 b=$2 want=${3:-}
for f in "$a" "$b"; do [[ -r $f ]] || { echo "cannot read $f" >&2; exit 2; }; done

normalize() {  # normalize <log> [cmd]
  awk -v want="${2:-}" '
    /^ctrl / {
      cmd = ""; size = ""; status = ""
      for (i = 1; i <= NF; i++) {
        if ($i ~ /^cmd=/) cmd = substr($i, 5)
        else if ($i ~ /^size=/) size = $i
        else if ($i ~ /^status=/) status = $i
      }
      keep = (want == "" || cmd == want)
      if (keep) print cmd " #" (++n[cmd]) " " size " " status
      next
    }
    /^(alloc|#)/ { keep = 0; next }
    keep { print }
  ' "$1"
}

if [[ -n $want ]]; then
  diff <(normalize "$a" "$want") <(normalize "$b" "$want") && echo "no difference in $want"
else
  diff <(normalize "$a") <(normalize "$b") && echo "no difference"
fi
