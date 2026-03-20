#!/bin/sh
set -eu

export PATH=/bin:/sbin

mkdir -p /proc /sys /dev /run
mount -t proc proc /proc || true
mount -t sysfs sysfs /sys || true
mount -t devtmpfs devtmpfs /dev || true
mkdir -p /dev/pts
mount -t devpts devpts /dev/pts || true
mount -t tmpfs tmpfs /run || true

# Best effort: load common virtio drivers if modules are present.
modprobe virtio_mmio 2>/dev/null || true
modprobe virtio_pci 2>/dev/null || true
modprobe virtio_blk 2>/dev/null || true
modprobe virtio_net 2>/dev/null || true
modprobe virtio_console 2>/dev/null || true

# Keep this initramfs minimal and interactive by default.
# IMPORTANT: PID 1 must not just exit, otherwise Linux panics with
# "Attempted to kill init". Prefer hvc0 (virtio console), then fall back.
TTY_DEV=
for dev in /dev/hvc0 /dev/ttyS0 /dev/console; do
  for _ in 1 2 3 4 5; do
    if [ -c "$dev" ]; then
      TTY_DEV="$dev"
      break
    fi
    sleep 1
  done
  [ -n "$TTY_DEV" ] && break
done

if [ -n "$TTY_DEV" ]; then
  exec <"$TTY_DEV" >"$TTY_DEV" 2>&1
fi

set +e
if command -v cttyhack >/dev/null 2>&1 && command -v setsid >/dev/null 2>&1; then
  setsid cttyhack sh
else
  sh
fi

exec poweroff -f
