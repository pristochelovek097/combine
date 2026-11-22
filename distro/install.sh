#!/bin/bash
set -euo pipefail
[[ -e /dev/qemu-vm ]] || { echo "Installer must run inside QEMU." >&2; exit 1; }
[[ "${1:-}" == --disk ]] || { echo "Usage: combine-install --disk /dev/vda|/dev/sda" >&2; exit 2; }
DISK="${2:-}"
case "$DISK" in /dev/vda|/dev/sda) ;; *) echo "Refusing disk: $DISK" >&2; exit 2;; esac
[[ -b "$DISK" ]] || { echo "Not a block device: $DISK" >&2; exit 1; }
cat <<EOF
WARNING: ERASE all data on QEMU virtual disk $DISK.
Type exactly ERASE $DISK to continue:
EOF
read -r confirmation
[[ "$confirmation" == "ERASE $DISK" ]] || { echo "Cancelled."; exit 1; }

umount "${DISK}"?* 2>/dev/null || true
printf 'label: gpt\n,1G,U,*\n,,L,\n' | sfdisk "$DISK"
partprobe "$DISK" 2>/dev/null || true
sleep 2
EFI="${DISK}1"; ROOTPART="${DISK}2"
mkfs.fat -F32 "$EFI"
mkfs.ext4 -F "$ROOTPART"
mkdir -p /mnt/combine-install
mount "$ROOTPART" /mnt/combine-install
mkdir -p /mnt/combine-install/boot/efi
mount "$EFI" /mnt/combine-install/boot/efi
# ISO initramfs has the complete live root packed in /run/live-root.cpio.gz.
mkdir -p /mnt/combine-install/.installer-root
zcat /run/live-root.cpio.gz | (cd /mnt/combine-install/.installer-root && cpio -idm --quiet)
cp -a /mnt/combine-install/.installer-root/. /mnt/combine-install/
rm -rf /mnt/combine-install/.installer-root
UUID_ROOT="$(blkid -s UUID -o value "$ROOTPART")"
UUID_EFI="$(blkid -s UUID -o value "$EFI")"
cat > /mnt/combine-install/etc/fstab <<EOF
UUID=$UUID_ROOT / ext4 defaults 0 1
UUID=$UUID_EFI /boot/efi vfat umask=0077 0 2
proc /proc proc defaults 0 0
sysfs /sys sysfs defaults 0 0
devtmpfs /dev devtmpfs defaults 0 0
tmpfs /run tmpfs defaults 0 0
EOF
mkdir -p /mnt/combine-install/dev /mnt/combine-install/proc /mnt/combine-install/sys /mnt/combine-install/run
grub-install --target=i386-pc --boot-directory=/mnt/combine-install/boot "$DISK"
cat > /mnt/combine-install/boot/grub/grub.cfg <<EOF
set timeout=3
set default=0
menuentry "COMBINE Linux" {
    search --no-floppy --fs-uuid --set=root $UUID_ROOT
    linux /boot/vmlinuz root=UUID=$UUID_ROOT ro quiet runlevel=5
    initrd /boot/initrd.img
}
menuentry "COMBINE Linux (console only)" {
    search --no-floppy --fs-uuid --set=root $UUID_ROOT
    linux /boot/vmlinuz root=UUID=$UUID_ROOT ro quiet runlevel=3
    initrd /boot/initrd.img
}
EOF
sync
umount -R /mnt/combine-install
echo "Installed COMBINE on $DISK. Power off, detach ISO, boot the virtual disk."
