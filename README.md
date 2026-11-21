# Rodnee's Kernel (`kernel_xiaomi_rosemary_rodnee`)

Custom Linux 4.19 (`lineage-24.0` / `4.19.325-cip136-st20-Rodnees-Kernel+`) kernel build kit and native Rust `aarch64` runtime tuner for the **Xiaomi Redmi Note 10S (`rosemary`, MediaTek Helio G95 / MT6785)** running **Shinkai Android 16**.

Tuned for **locked 60 fps smoothness**, **full MediaTek hardware acceleration**, **low I/O & network latency**, and **cool daily battery efficiency**, with **KernelSU (`v3.3.0-55`)** built in and a **100% stock FocalTech `ft3418` touchscreen driver**.

---

## Highlights

- **Base Tree:** [`xiaomi-mt6785-dev/android_kernel_xiaomi_mt6785`](https://github.com/xiaomi-mt6785-dev/android_kernel_xiaomi_mt6785) (`lineage-24.0`, `4.19.325-cip136-st20`)
- **Compiler & Linker:** Android NDK r27c Clang `18.0.3` + `LLD 18.0.3` with **Full LTO** (`CONFIG_LTO_CLANG=y`, `# CONFIG_THINLTO is not set`) and **`CONFIG_ARM64_LSE_ATOMICS=y`**
- **Hardware Acceleration (100% intact):**
  - ARM Mali-G76 MC4 (`13040000.mali`, DDK `r32p1-00bet5`) + MediaTek `ged` DVFS/boost
  - Native `1080×2400 @ 60.0 Hz` display pipeline (`mtk_disp`, `cmdq`)
  - Cherry-picked `mt6785` display repaint-queue fixes (`Recheck the repaint queue after waking` & unlock on allocation failure)
  - 100% stock FocalTech touch driver (`Focaltech V3.2 20200422`, `fts_ts`, `chip id = 0x5452`)
- **Low-Latency Scheduler & Idle Config:**
  - `CONFIG_PREEMPT=y`, `HZ=250`, `CONFIG_UCLAMP_TASK=y`, `CONFIG_UCLAMP_TASK_GROUP=y`
  - Disabled scheduler tracing & CFS fair-group overhead (`FAIR_GROUP_SCHED=n`, `MTK_SCHED_TRACERS=n`, `SCHEDSTATS=n`, `SCHED_INFO=n`)
  - `CONFIG_CPU_IDLE_NO_MTK=y` (`menu` governor — eliminates 150 µs `mcdi` wakeup stalls), `CONFIG_WQ_POWER_EFFICIENT_DEFAULT=y`
  - Fixed `mtktsbtspdchg` double-ADC thermal reading bug (`lineage-24.0`), removed spurious WiFi `gen4m` CPU boost, disabled TrustZone logging overhead
- **I/O, ZRAM & Networking:**
  - `deadline` default I/O scheduler (`CONFIG_DEFAULT_IOSCHED="deadline"`)
  - TCP **BBR** default (`CONFIG_DEFAULT_TCP_CONG="bbr"`) + **`CONFIG_NET_SCH_FQ=y`**
  - `ZRAM` with **`LZ4`** compression (`CONFIG_CRYPTO_LZ4=y`)
- **Root:** Integrated [`backslashxx/KernelSU`](https://github.com/backslashxx/KernelSU) `v3.3.0-55` (`CONFIG_KSU=y`, `SELinux=Enforcing`)
- **Native Rust Tuner (`rodnee-tuner`):** Tiny `320 KB` `aarch64-linux-android` binary (~`4.1 MB` RSS, `0.0%` CPU) that applies baseline low-latency I/O, VM (`page-cluster=0`), BBR/ECN/TFO, and dynamically switches between **Screen ON** (locked 60 fps low-latency) and **Screen OFF** (deep standby battery-saver) profiles.

---

## Repository Layout

```text
.
├── configs/
│   └── rodnees_rosemary.config          # Exact kernel .config (/proc/config.gz)
├── patches/
│   ├── 01-mt6785-repaint-unlock.patch   # Unlock repaint queue on alloc failure
│   ├── 02-mt6785-repaint-recheck.patch  # Recheck repaint queue after waking
│   ├── 03-mcdi-no-mtk-menu-guard.patch  # Guard get_menu_* when CPU_IDLE_NO_MTK=y
│   └── 04-openssl4-host-compat.patch    # Host OpenSSL 4.x compatibility for scripts/
├── rodnee-tuner/
│   ├── Cargo.toml
│   ├── src/main.rs                      # Native Rust aarch64 runtime tuner
│   └── 02_rodnee_tuner.sh               # KernelSU service.d launcher
└── scripts/
    ├── build.sh                         # End-to-end reproducible kernel build script
    └── bc                               # Minimal Python bc helper for timeconst.bc
```

---

## Prebuilt Release (`v2.0`)

Prebuilt binaries are attached under **[Releases](https://github.com/eendor/kernel_xiaomi_rosemary_rodnee/releases)**:

- `shinkai_rosemary_boot_a_rodnees_kernel.img` — Ready-to-flash 64 MiB `boot_a` image for Shinkai Android 16 on `rosemary`
- `rodnee-tuner` — Cross-compiled `aarch64-linux-android` Rust tuner binary
- `SHA256SUMS.txt` — Verification checksums

### Flashing Warning & Instructions

> **Warning:** Always back up your current `boot` partition first and verify your active A/B slot (`getprop ro.boot.slot_suffix`). Do **not** overwrite your recovery slot (`boot_b` if you keep OrangeFox/TWRP there).

1. **Flash `boot_a` (with root / ADB or fastboot):**
   ```bash
   # Via fastboot:
   fastboot flash boot_a shinkai_rosemary_boot_a_rodnees_kernel.img
   fastboot reboot

   # Or on-device (root, when active slot is _a):
   dd if=/data/local/tmp/shinkai_rosemary_boot_a_rodnees_kernel.img of=/dev/block/by-name/boot_a bs=4096
   sync && reboot
   ```

2. **Install `rodnee-tuner` (optional, recommended):**
   ```bash
   adb push rodnee-tuner /data/local/tmp/rodnee-tuner
   adb push rodnee-tuner/02_rodnee_tuner.sh /data/local/tmp/02_rodnee_tuner.sh
   adb shell "su -c '
     mkdir -p /data/adb/rodnee
     cp /data/local/tmp/rodnee-tuner /data/adb/rodnee/rodnee-tuner
     chmod 0755 /data/adb/rodnee/rodnee-tuner
     cp /data/local/tmp/02_rodnee_tuner.sh /data/adb/service.d/02_rodnee_tuner.sh
     chmod 0755 /data/adb/service.d/02_rodnee_tuner.sh
     rm -f /data/local/tmp/rodnee-tuner /data/local/tmp/02_rodnee_tuner.sh
   '"
   ```

---

## Building From Source

### Requirements
- Linux x86_64 host with `git`, `make`, `bison`, `flex`, `cpio`, `perl`, `python3`, `openssl`
- Android NDK r27c (Clang `18.0.3` + `LLD 18.0.3`), set via `NDK_BIN`

### Build Kernel
```bash
export NDK_BIN="$HOME/Android/Sdk/ndk/27.2.12479018/toolchains/llvm/prebuilt/linux-x86_64/bin"
./scripts/build.sh
```
The compiled `Image` and `Image.gz` will be output in `build/kernel/out/arch/arm64/boot/`.

### Cross-Compile `rodnee-tuner`
```bash
rustc --edition=2021 --target aarch64-linux-android \
  -C linker="$NDK_BIN/aarch64-linux-android35-clang" \
  -C opt-level=z -C lto=fat -C panic=abort -C strip=symbols -C target-cpu=cortex-a55 \
  -o rodnee-tuner/rodnee-tuner rodnee-tuner/src/main.rs
```

---

## Credits
- [`xiaomi-mt6785-dev/android_kernel_xiaomi_mt6785`](https://github.com/xiaomi-mt6785-dev/android_kernel_xiaomi_mt6785) — MT6785 `lineage-24.0` kernel base
- [`skyyverse/kernel_xiaomi_rosemary`](https://github.com/skyyverse/kernel_xiaomi_rosemary) — `mt6785` display repaint-queue patches
- [`backslashxx/KernelSU`](https://github.com/backslashxx/KernelSU) — KernelSU `v3.3.0-55`
