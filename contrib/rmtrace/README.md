# rmtrace

An `LD_PRELOAD` logger for the NVIDIA RM control calls NVML makes on Linux. Running `nvoc` under it records the command
IDs and parameter bytes the driver receives for each operation: clock offsets, locked clocks and power limits. This is
how to learn the private controls NVML uses. A driverless host can then reproduce them, for example macuda, which sends
RM controls straight to the GSP firmware on macOS.

nvoc itself is unchanged; this only observes it.

## Build

```bash
make            # -> rmtrace.so
```

## Capture a sweep

```bash
sudo NVOC=$(command -v nvoc) ./capture.sh -d 0
```

The script runs `nvoc` once per setting and writes one log per step:

- info
- two GPU offsets
- two memory offsets
- two power limits
- two locked-clock ranges
- reset

Each step changes exactly one value. The defaults are mild (+15/+30 MHz, +100/+200 MHz memory, 90/100 % power,
down-clock locks). Override the values through `GPU_OFFSETS`, `MEM_OFFSETS`, `POWERS` and `CLOCKS`.

The run always ends with `nvoc reset`, including on Ctrl-C. That puts the card back at **driver defaults, not your
profile**, so re-apply your settings afterwards (for example by restarting `nvoc-apply.service`).

`-d` is nvoc's device index, which can change across reboots; check `nvoc info` first. Capture once per architecture you
want to replay on, for example a Blackwell card and the 4090, because the layouts may differ.

Output goes to `rmtrace-<timestamp>/`:

- `meta.txt`: driver version and GPU name.
- `NN-<step>.log`: every RM allocation and control call nvoc made in that step.
- `summary.txt`: the command IDs each step issued that `info` did not. The setter you are looking for should be in there.

Every log line carries a timestamp, a sequence number and kernel-assigned handles, so a plain `diff` of two logs is all
noise. `rmdiff.sh` strips those and compares only the parameter words:

```bash
./rmdiff.sh rmtrace-*/01-gpu-offset-15.log rmtrace-*/02-gpu-offset-30.log 0x2080xxxx   # one command from summary.txt
./rmdiff.sh rmtrace-*/01-gpu-offset-15.log rmtrace-*/02-gpu-offset-30.log              # every command
```

Look for a word that tracks the setting between the two runs. It may not be the raw number: the driver can store it
scaled (kHz instead of MHz, or doubled for memory) or encoded. Words that differ in every comparison, including
`info`-only commands, are live readings such as temperature and power, not settings.

## One-off use

```bash
sudo env LD_PRELOAD=$PWD/rmtrace.so RMTRACE_OUT=/tmp/oc.log nvoc -d 0 -o 100
```

Plain `sudo LD_PRELOAD=...` does not work, because sudo strips `LD_*` variables; go through `env`.

| Variable        | Meaning                                    | Default |
|-----------------|--------------------------------------------|---------|
| `RMTRACE_OUT`   | log file (appended to)                     | stderr  |
| `RMTRACE_MAX`   | max parameter bytes dumped per call, 0 = all | 4096 (`capture.sh` uses 0) |
| `RMTRACE_ALLOC` | `0` hides allocation lines                 | 1       |

## Log format

```
alloc #1 t=0.000031 class=0x2080 client=0xc1d00001 parent=0xc1d00001 new=0x5c000003 size=8 status=0x0
ctrl #2 t=0.000037 cmd=0x20802096 client=0xc1d00001 object=0x5c000003 class=0x2080 flags=0x0 size=38 status=0x0
  in  0000: 00000000 000000c8 ffffff9c 00000003 00000004 00000005 00000006 00000007
  out 0000: ...            (only when the driver changed the block; otherwise "out unchanged")
```

- Parameter blocks are little-endian 32-bit words, eight per line, prefixed with their byte offset.
- `class` is resolved from earlier allocations. `0x2080` is the subdevice, where perf, clock and power controls live.
- `status` is the RM status after the call; `0x0` is success.

## Caveats

- **Match the driver branch to the replay target.** Private parameter layouts and command IDs change between driver
  branches. macuda runs GSP firmware 570.144, so capture on a 570.x driver; `capture.sh` warns otherwise.
- **Embedded pointers are not followed.** Parameter blocks are dumped flat. If a control's parameters contain a pointer
  to a list, the log shows the pointer value, not the list.
- **FINN-serialized calls look different.** Calls marked `flags=0x4(finn)` carry a serialized layout, not the raw
  struct.
- **This shows what reaches the kernel driver, not what reaches GSP.** On GSP cards the clock and power logic runs in
  the firmware, so these controls are expected to be forwarded to it. That is unverified, and the driver may rewrite
  some calls on the way. Treat the log as the starting point for a replay, not proof that it will work.
- **An empty log** (just the `# rmtrace` header) means the library loaded but saw no NVIDIA control calls. A missing log
  means it never loaded. `capture.sh` warns about both.
