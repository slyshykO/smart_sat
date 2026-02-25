# smart_sat

Minimal `no_std` Rust utility to read ATA SMART attributes through Linux SG_IO (SAT ATA PASS-THROUGH 16).

## What it does

- Opens a block device (for example `/dev/sda`).
- Sends ATA `SMART READ VALUES` (`0xB0/0xD0`) via `SG_IO`.
- Parses SMART attribute table and prints selected IDs:
  - `5` Reallocated Sectors Count
  - `9` Power-On Hours
  - `194` Temperature Celsius (commonly disk temperature)
  - `197` Current Pending Sector Count
  - `198` Offline Uncorrectable

## Requirements

- Linux host
- Rust toolchain (`rustup`, `cargo`)
- Zig binary available under `~/bin/zig*` (or set `ZIG_BIN=/path/to/zig`)
- Root privileges on target system to access raw disk/SG ioctls

## Build

This project includes a helper script:

- `scripts/build-with-zig.sh`

Supported targets:

- `arm-unknown-linux-musleabihf`
- `arm-unknown-linux-gnueabihf`

Examples:

```bash
./scripts/build-with-zig.sh arm-unknown-linux-musleabihf --release
./scripts/build-with-zig.sh arm-unknown-linux-gnueabihf --release
```

Notes:

- The script auto-runs `rustup target add <target>`.
- For `musleabihf`, it applies a `RUSTFLAGS` workaround for Zig+musl CRT behavior.

## Run

On the target device:

```bash
./smart_sat /dev/sdX
```

Example:

```bash
./smart_sat /dev/sda
```

## Copy to target

Example SCP command:

```bash
scp -O ./target/arm-unknown-linux-musleabihf/release/smart_sat root@<target-ip>:/root
```

## Exit codes

- `0` success
- `2` bad usage
- `3` open failed
- `4` `ioctl(SG_IO)` failed
- `5` ATA error status reported
- `6` abnormal SG status without ATA return descriptor
