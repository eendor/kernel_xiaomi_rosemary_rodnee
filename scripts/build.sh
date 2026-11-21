#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_DIR="${BUILD_DIR:-$ROOT_DIR/build}"
NDK_BIN="${NDK_BIN:-$HOME/Android/Sdk/ndk/27.2.12479018/toolchains/llvm/prebuilt/linux-x86_64/bin}"

if [ ! -x "$NDK_BIN/clang" ]; then
  echo "ERROR: Android NDK clang not found at $NDK_BIN/clang"
  echo "Set NDK_BIN=/path/to/ndk/toolchains/llvm/prebuilt/linux-x86_64/bin"
  exit 1
fi

mkdir -p "$BUILD_DIR"
cd "$BUILD_DIR"

if [ ! -d kernel ]; then
  git clone --depth=1 -b lineage-24.0 https://github.com/xiaomi-mt6785-dev/android_kernel_xiaomi_mt6785.git kernel
fi

if [ ! -d KernelSU ]; then
  git clone --depth=1 -b v3.3.0-55 https://github.com/backslashxx/KernelSU.git KernelSU
fi

cd kernel

for p in "$ROOT_DIR"/patches/*.patch; do
  if git apply --check "$p" 2>/dev/null; then
    echo "Applying $(basename "$p")..."
    git apply "$p"
  else
    echo "Skipping $(basename "$p") (already applied or not needed)"
  fi
done

ln -sfn "$BUILD_DIR/KernelSU/kernel" drivers/kernelsu
grep -q "kernelsu" drivers/Makefile || printf '\nobj-$(CONFIG_KSU) += kernelsu/\n' >> drivers/Makefile
grep -q 'drivers/kernelsu/Kconfig' drivers/Kconfig || sed -i '$i source "drivers/kernelsu/Kconfig"' drivers/Kconfig

export PATH="$ROOT_DIR/scripts:$NDK_BIN:$PATH"
export KBUILD_BUILD_USER="${KBUILD_BUILD_USER:-rodnee}"
export KBUILD_BUILD_HOST="${KBUILD_BUILD_HOST:-rodnees-kernel}"

MAKE_ARGS=(
  O=out ARCH=arm64 CC=clang LLVM=1 LLVM_IAS=1
  LD=ld.lld AR=llvm-ar NM=llvm-nm
  OBJCOPY=llvm-objcopy OBJDUMP=llvm-objdump STRIP=llvm-strip
  CROSS_COMPILE=aarch64-linux-gnu- CROSS_COMPILE_ARM32=arm-linux-gnueabi-
)

make "${MAKE_ARGS[@]}" rosemary_defconfig

./scripts/config --file out/.config \
  --set-str LOCALVERSION "-Rodnees-Kernel" \
  --disable LOCALVERSION_AUTO \
  --enable CC_OPTIMIZE_FOR_PERFORMANCE \
  --enable LTO \
  --enable LTO_CLANG \
  --disable THINLTO \
  --enable ARM64_LSE_ATOMICS \
  --disable DEBUG_INFO \
  --disable CPU_IDLE_NO_MTK \
  --enable ARM_MTK_CPUIDLE \
  --enable CPU_IDLE_GOV_MTK_MENU \
  --enable WQ_POWER_EFFICIENT_DEFAULT \
  --enable PREEMPT \
  --enable CGROUP_SCHED \
  --disable FAIR_GROUP_SCHED \
  --enable UCLAMP_TASK \
  --enable UCLAMP_TASK_GROUP \
  --disable MTK_SCHED_TRACERS \
  --disable SCHEDSTATS \
  --disable SCHED_INFO \
  --enable IOSCHED_DEADLINE \
  --enable DEFAULT_DEADLINE \
  --disable DEFAULT_CFQ \
  --set-str DEFAULT_IOSCHED "deadline" \
  --enable TCP_CONG_BBR \
  --enable DEFAULT_BBR \
  --disable DEFAULT_CUBIC \
  --disable DEFAULT_BIC \
  --set-str DEFAULT_TCP_CONG "bbr" \
  --enable NET_SCH_FQ \
  --enable ZRAM \
  --enable CRYPTO_LZ4 \
  --enable KSU \
  --enable KSU_TAMPER_SYSCALL_TABLE \
  --enable KSU_LSM_SECURITY_HOOKS \
  --enable KSU_FEATURE_SULOG \
  --enable KSU_FEATURE_ADBROOT

make "${MAKE_ARGS[@]}" olddefconfig
make -j"$(nproc)" "${MAKE_ARGS[@]}" Image.gz

echo "Build complete:"
ls -lh out/arch/arm64/boot/Image out/arch/arm64/boot/Image.gz
