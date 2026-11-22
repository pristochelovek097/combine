#!/usr/bin/env bash
# Create/use a disposable QEMU disk image under OUT; never accepts a host
# block-device path. Boots the live ISO with the virtual disk attached.
set -euo pipefail
PROJECT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${OUT:-/tmp/opencode/combine-build}"
ISO="${ISO:-$OUT/combine.iso}"
DISK="${DISK:-$OUT/combine.qcow2}"
[[ -f "$ISO" ]] || { echo "ISO missing; run distro/build.sh first" >&2; exit 1; }
case "$DISK" in /dev/*) echo "Refusing block-device target: $DISK" >&2; exit 2;; esac
mkdir -p "$(dirname "$DISK")"
if [[ ! -e "$DISK" ]]; then
    qemu-img create -f qcow2 "$DISK" 16G
else
    [[ "$(qemu-img info --output=json "$DISK" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("format", ""))')" == qcow2 ]] || {
        echo "Refusing existing non-qcow2 disk: $DISK" >&2; exit 2;
    }
fi
exec qemu-system-x86_64 -m "${MEMORY:-1024}" -vga std -global VGA.vgamem_mb=32 \
    -nic user,model=virtio-net-pci -cdrom "$ISO" -boot d \
    -drive "file=$DISK,if=virtio,format=qcow2"
