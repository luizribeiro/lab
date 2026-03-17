#!/bin/sh
set -eu

export PATH=/bin:/sbin

echo "[capsa-initramfs] booting minimal userspace"

mkdir -p /proc /sys /dev /run
mount -t proc proc /proc || true
mount -t sysfs sysfs /sys || true
mount -t devtmpfs devtmpfs /dev || true
mount -t tmpfs tmpfs /run || true

# Best effort: load common virtio drivers if modules are present.
modprobe virtio_mmio 2>/dev/null || true
modprobe virtio_pci 2>/dev/null || true
modprobe virtio_blk 2>/dev/null || true
modprobe virtio_net 2>/dev/null || true
modprobe virtio_console 2>/dev/null || true

# Keep this initramfs minimal and interactive by default.
# IMPORTANT: PID 1 must not just exit, otherwise Linux panics with
# "Attempted to kill init". Run a shell, then power off the VM cleanly.
sh

echo "[capsa-initramfs] shell exited, powering off VM"
poweroff -f || reboot -f

# Fallback: keep PID 1 alive if shutdown syscall isn't available.
while true; do sleep 3600; done
