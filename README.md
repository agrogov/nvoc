# NVOC - NVIDIA GPU Overclocking

GPU overclocking/undervolting utility for Blackwell RTX 50-series on Linux.

Born out of my frustration with the lack of an API that is both easy to use in the terminal, and easy to script around.

## Requirements

- Linux x86_64
- RTX 50-series GPU (5090, 5080, 5070, 5060)
- nvidia-open 550+ driver
- nvidia-utils package
- Root access

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

Power limits are percentages of the GPU's default power limit. Hardware enforces absolute min/max constraints regardless of percentage.

### Info

```
$ nvoc info
driver: 595.58.03
+-----+----------------------------+---------+---------+---------+---------+------+-------+-------+-------+----------------------+
| idx | name                       | gpu_clk | gpu_off | mem_clk | mem_off | temp | power | pwr_w | pwr_% | pwr_range            |
+-----+----------------------------+---------+---------+---------+---------+------+-------+-------+-------+----------------------+
|   0 | NVIDIA GeForce RTX 5060 Ti |     577 |     400 |     405 |    6000 |   33 |     2 |   180 |   100 | 150-180W (hard 198W) |
|   1 | NVIDIA GeForce RTX 5060 Ti |     577 |     400 |     405 |    6000 |   34 |     3 |   180 |   100 | 150-180W (hard 198W) |
|   2 | NVIDIA GeForce RTX 5090    |     525 |     350 |     405 |    6000 |   44 |    20 |   575 |   100 | 400-575W (hard 600W) |
|   3 | NVIDIA GeForce RTX 4090    |     210 |       0 |     405 |       0 |   38 |    22 |   450 |   100 | 150-450W (hard 450W) |
+-----+----------------------------+---------+---------+---------+---------+------+-------+-------+-------+----------------------+
```

### Monitor

```bash
watch -n 1 nvoc info
```

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
