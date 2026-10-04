# NVOC - NVIDIA GPU Overclocking

GPU overclocking and undervolting utility for NVIDIA Blackwell GPUs on Linux, with experimental Ada Lovelace support (Help wanted with Ampere GPU support, see [Issues](https://github.com/martinstark/nvoc/issues)).

Supports single and multi GPU setups. Filter GPUs by string matching, regex, or uuid.

Born out of my frustration with the lack of an API that is both easy to use in the terminal, and easy to script around.

## Requirements

- Linux
- Blackwell GPU (GeForce RTX 50-series or RTX PRO Blackwell) or experimental Ada Lovelace GPU (GeForce RTX 40-series)
- nvidia-open 555+ driver
- nvidia-utils package
- root access

## Install

### AUR (Arch Linux)

```bash
paru -S nvoc-cli
```

### From source

```bash
# Install dependencies (Arch Linux)
sudo pacman -S nvidia-open nvidia-utils

# Build and install
cargo build --release
sudo cp target/release/nvoc /usr/local/bin/
```

### Docker artifact (Linux x86_64)

Build `dist/nvoc` without installing Rust locally:

```bash
mkdir -p dist
docker buildx build --platform linux/amd64 --target artifact --output type=local,dest=dist .
```

## Usage

```bash
# Show GPU information (all GPUs, table output)
nvoc info

# Show a single GPU (legacy text output)
nvoc info -d 2

# Target GPUs by name (regex)
nvoc info --match '.*RTX 5090.*'

# Apply OC settings to a specific GPU
sudo nvoc -d 0 -c MIN,MAX -o OFFSET -m MEM_OFFSET -p POWER_LIMIT

# Apply offsets to all matching GPUs
sudo nvoc --match '.*RTX 5090.*' -o 400 -m 6000

# Reset
sudo nvoc reset

# Dry Run (table output even for a single GPU unless `--no-table` is set)
sudo nvoc --match '.*RTX 5090.*' -c 200,2800 --dry-run
```

### Options

- `-c, --clocks <MIN,MAX>` - Set GPU locked clocks (MHz)
- `-o, --offset <OFFSET>` - Graphics clock offset (MHz)
- `-m, --memory-offset <OFFSET>` - Memory clock offset (MHz)
- `-p, --power <PERCENT>` - Power limit percentage (50-150%)
- `-d, --device <INDEX>` - GPU device index (default: 0 for apply/reset; `info` defaults to all GPUs)
- `--match <REGEX>` - Target GPUs by device name regex
- `--all` - Target all GPUs
- `--dry-run` - Preview changes only
- `--no-table` - Disable table output

### Examples

```bash
# SINGLE GPU

# 5090 uv example
sudo nvoc -c 200,2820 -o 856 -m 2000 -p 105

# Graphics offset
sudo nvoc -o 200

# Memory offset
sudo nvoc -m 1500

# Power limit
sudo nvoc -p 105

# Locked clocks
sudo nvoc -c 200,2800
```

```bash
# MULTI GPU

# Apply to all GPUs
sudo nvoc -d all -o 200

# Using Device ID (unstable across reboots)
sudo nvoc -d 0,1 -c 200,2820 -o 856 -m 2000 -p 105

# Using UUID (stable across reboots)
sudo nvoc -d GPU-1234... -o 856

# Using name pattern (case-insensitive substring match on device name)
sudo nvoc -d name:5090 -o 856
sudo nvoc -d "n:5060 ti" -o 100

# Using a regex pattern (case-sensitive regex search on device name)
sudo nvoc -d "regex:RTX 50[89]0" -o 856
# Optional ' Ti' suffix - matches both "RTX 5060" and "RTX 5060 Ti"
sudo nvoc -d "r:5060( Ti)?" -o 100
```

Power limits are percentages of the GPU's default power limit. `nvoc` does not enforce a fixed percentage range; the card's NVML min/max power limits decide the effective watts.

### Info

```
$ nvoc info
driver: 595.58.03
+-----+------------------------------------------+---------+---------+---------+---------+------+-------+-------+-------+----------------------+
| idx | name                                     | gpu_clk | gpu_off | mem_clk | mem_off | temp | power | pwr_w | pwr_% | pwr_range            |
+-----+------------------------------------------+---------+---------+---------+---------+------+-------+-------+-------+----------------------+
|   0 | NVIDIA GeForce RTX 5060 Ti               |     577 |     400 |     405 |    6000 |   33 |     2 |   180 |   100 | 150-180W (hard 198W) |
|   1 | NVIDIA GeForce RTX 5060 Ti               |     577 |     400 |     405 |    6000 |   34 |     3 |   180 |   100 | 150-180W (hard 198W) |
|   2 | NVIDIA GeForce RTX 5090                  |     525 |     350 |     405 |    6000 |   44 |    20 |   575 |   100 | 400-575W (hard 600W) |
|   3 | NVIDIA RTX PRO 4000 Blackwell SFF Editi… |     480 |     300 |     405 |    4000 |   31 |     7 |    70 |   100 | 60-70W (hard 70W)    |
|   4 | NVIDIA GeForce RTX 4090                  |     210 |     100 |     405 |    3000 |   41 |    35 |   360 |    80 | 150-450W (hard 450W) |
+-----+------------------------------------------+---------+---------+---------+---------+------+-------+-------+-------+----------------------+
```

```bash
# JSON
nvoc info --json
```

### List

```
$ nvoc list
0 - NVIDIA GeForce RTX 5090 - GPU-1234...
```

```bash
# UUIDs only, separated by line break, no labels
nvoc list --uuid

# JSON
nvoc list --json
```

### Monitor

```bash
watch -n 1 nvoc info
```

### Apply on Boot (systemd)

To apply settings on every boot, install a oneshot service:

```ini
# /etc/systemd/system/gpu-oc.service
[Unit]
Description=GPU overclock settings
After=multi-user.target

[Service]
Type=oneshot
ExecStart=/usr/bin/nvoc -c 200,2820 -o 856 -m 2000 -p 105

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now gpu-oc.service
```

Replace the `ExecStart` arguments with your tuned values. Adjust the binary path to `/usr/local/bin/nvoc` if you installed from source.

On multi-GPU systems, pin by UUID (`-d GPU-...`) or by model (`-d name:5090`, or `-d "r:50[89]0"` for a regex match). NVML device indices aren't guaranteed stable across reboots.

## Limitations

The NVML API only supports global clock offsets, not per-voltage-point adjustments. Fine-grained undervolting (setting a specific frequency at a specific voltage) is not possible. Tools like MSI Afterburner achieve this through a non-public API. This is an NVML limitation, not specific to `nvoc`.

## Boot auto-apply (systemd)

`nvoc` is designed to be run as a CLI. To auto-apply settings at boot, use a systemd oneshot unit with retries.

1. Install `nvoc` to a stable path (example):

```bash
sudo install -m 0755 dist/nvoc /usr/local/bin/nvoc
```

2. Copy the unit template and edit `ExecStart` for your GPUs/settings:

```bash
sudo install -m 0644 contrib/systemd/nvoc-apply.service /etc/systemd/system/nvoc-apply.service
sudo systemctl daemon-reload
```

Notes for editing `/etc/systemd/system/nvoc-apply.service`:

- `ExecStart=` is **not** a shell: quotes are not interpreted, and whitespace splits arguments.
- For regexes containing spaces, pass them as a single argv token using `--match=<REGEX>` and escape spaces, e.g. `--match=.*RTX\\s+5060\\s+Ti.*`.
- Multiple `ExecStart=` lines are allowed for `Type=oneshot` and run sequentially.

3. Safe rollout (dry-run first):

```bash
# Tip: add `--dry-run` to each `ExecStart=` line first.
sudo systemctl start nvoc-apply.service
journalctl -u nvoc-apply.service -b --no-pager
```

4. Enable at boot:

```bash
sudo systemctl enable --now nvoc-apply.service
```
