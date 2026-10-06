#!/usr/bin/env bash
# Run nvoc under rmtrace once per setting, one log per step, so each setting's RM control can be found by diffing.
#
#   sudo ./capture.sh [-d INDEX] [OUTDIR]
#
# Every step changes one value. The values are deliberately mild (small offsets, down-clocks, power at or below
# default) and the run ends with `nvoc reset`, also on Ctrl-C. Override any of them through the environment:
#   NVOC=/path/to/nvoc  GPU_OFFSETS="15 30"  MEM_OFFSETS="100 200"  POWERS="90 100"  CLOCKS="300,1500 300,1800"
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
dev=0
if [[ ${1:-} == -d ]]; then dev=$2; shift 2; fi
outdir=${1:-rmtrace-$(date +%Y%m%d-%H%M%S)}

NVOC=${NVOC:-$(command -v nvoc || true)}
GPU_OFFSETS=${GPU_OFFSETS:-"15 30"}
MEM_OFFSETS=${MEM_OFFSETS:-"100 200"}
POWERS=${POWERS:-"90 100"}
CLOCKS=${CLOCKS:-"300,1500 300,1800"}

[[ $EUID -eq 0 ]] || { echo "run as root (nvoc needs it): sudo $0 $*" >&2; exit 1; }
[[ -x $NVOC ]] || { echo "nvoc not found; set NVOC=/path/to/nvoc" >&2; exit 1; }
[[ -f $here/rmtrace.so ]] || make -C "$here" >/dev/null

mkdir -p "$outdir"
driver=$(nvidia-smi --query-gpu=driver_version --format=csv,noheader -i "$dev" 2>/dev/null || echo unknown)
name=$(nvidia-smi --query-gpu=name --format=csv,noheader -i "$dev" 2>/dev/null || echo unknown)
printf 'driver %s\ngpu %s (index %s)\ndate %s\n' "$driver" "$name" "$dev" "$(date -Is)" > "$outdir/meta.txt"
[[ $driver == 570.* ]] || echo "note: driver $driver - macuda runs GSP 570.144; private layouts may differ on other branches" >&2

step=0
run() {  # run <label> <nvoc args...>
  local label=$1; shift
  local log; log=$(printf '%s/%02d-%s.log' "$outdir" "$step" "$label")
  step=$((step + 1))
  echo "== $label: nvoc $*"
  # RMTRACE_MAX=0: dump whole parameter blocks; a per-domain array can put the changing field past any fixed cap.
  env LD_PRELOAD="$here/rmtrace.so" RMTRACE_OUT="$log" RMTRACE_MAX=0 "$NVOC" "$@" || echo "   (nvoc exited $?)"
  if [[ ! -s $log ]]; then echo "   warning: no log - rmtrace.so did not load" >&2
  elif ! grep -q '^ctrl ' "$log"; then echo "   warning: rmtrace loaded but saw no RM control calls" >&2
  fi
}

reminder() { echo "== the card is back at driver defaults; re-apply your own profile (e.g. nvoc-apply.service)"; }
trap 'echo "== resetting"; "$NVOC" reset -d "$dev" || true; reminder' EXIT

run info info -d "$dev"
for v in $GPU_OFFSETS; do run "gpu-offset-$v" -d "$dev" -o "$v"; done
for v in $MEM_OFFSETS; do run "mem-offset-$v" -d "$dev" -m "$v"; done
for v in $POWERS;      do run "power-$v" -d "$dev" -p "$v"; done
for v in $CLOCKS;      do run "clocks-${v/,/-}" -d "$dev" -c "$v"; done
run reset reset -d "$dev"
trap - EXIT

# Commands each step issues that `info` does not: the setter (and whatever it reads first) stands out here.
{
  base=$(grep -o 'cmd=0x[0-9a-f]*' "$outdir"/00-info.log 2>/dev/null | sort -u || true)
  for log in "$outdir"/[0-9][0-9]-*.log; do
    [[ -e $log && $log != */00-info.log ]] || continue
    echo "## $(basename "$log" .log)"
    { grep -o 'cmd=0x[0-9a-f]*' "$log" || true; } | sort | uniq -c | while read -r n c; do
      grep -qx "$c" <<<"$base" || echo "  $c x$n"
    done
  done
} > "$outdir/summary.txt"
echo "logs in $outdir; new commands per step in $outdir/summary.txt"
echo "compare two runs of one setter: $here/rmdiff.sh $outdir/01-*.log $outdir/02-*.log <cmd>"
reminder
