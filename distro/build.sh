#!/usr/bin/env bash
# Build a bootable COMBINE ISO: COMBINE oneinit + glibc + GNU coreutils.
#
# Everything is assembled in $OUT (default /tmp/opencode/combine-build).
# No root privileges required.
set -euo pipefail

PROJECT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${OUT:-/tmp/opencode/combine-build}"
KERNEL="${KERNEL:-$(ls -1 /boot/vmlinuz-* 2>/dev/null | sort -V | tail -n1)}"
BUSYBOX_URL="${BUSYBOX_URL:-https://raw.githubusercontent.com/pristochelovek097/wpm-repo/main/pool/main/busybox}"
REPO_URL="${REPO_URL:-https://raw.githubusercontent.com/pristochelovek097/wpm-repo/main}"
ONEINIT_REPO="${ONEINIT_REPO:-https://github.com/pristochelovek097/oneinit.git}"
ROOT="$OUT/root"

[ -n "$KERNEL" ] && [ -e "$KERNEL" ] || { echo "no kernel found; set KERNEL=" >&2; exit 1; }

rm -rf "$OUT"
mkdir -p "$OUT"
mkdir -p "$ROOT"/{usr/bin,usr/sbin,usr/lib,etc/init.d,proc,sys,dev,run,tmp,root,var/log,var/cache,var/lib,home,opt,mnt}

# usr-merge: the glibc loader searches /usr/lib64, so expose it as a symlink.
ln -sfn usr/bin "$ROOT/bin"
ln -sfn usr/sbin "$ROOT/sbin"
ln -sfn usr/lib "$ROOT/lib"
ln -sfn usr/lib "$ROOT/lib64"
ln -sfn lib "$ROOT/usr/lib64"

echo "==> init (C, static) from $ONEINIT_REPO"
git clone -q --depth 1 "$ONEINIT_REPO" "$OUT/oneinit"
gcc -static -O2 -s -o "$ROOT/init" "$OUT/oneinit/init.c"

echo "==> busybox (init helper: getty/login/stty/ip/umount)"
curl -fsSL -o "$ROOT/bin/busybox" "$BUSYBOX_URL"
chmod 755 "$ROOT/bin/busybox"

copy_libs() {
    local elf="$1" interp lib
    [ -e "$elf" ] || return 0
    interp=$(readelf -l "$elf" 2>/dev/null | awk '/interpreter/{print $NF}' | tr -d ']' || true)
    if [ -n "${interp:-}" ]; then
        cp -Lf "$interp" "$ROOT/usr/lib/$(basename "$interp")"
    fi
    ldd "$elf" 2>/dev/null \
        | awk '{for (i = 1; i <= NF; i++) if ($i ~ /^\//) print $i}' \
        | while read -r lib; do
            [ -e "$lib" ] || continue
            cp -Lf "$lib" "$ROOT/usr/lib/$(basename "$lib")"
        done || true
}

install_gnu() {
    local name="$1" src
    src="$(command -v "$name" 2>/dev/null || true)"
    case "$src" in /*) ;; *) return 0 ;; esac
    cp -Lf "$src" "$ROOT/bin/$name"
    chmod 755 "$ROOT/bin/$name"
    copy_libs "$src"
}

echo "==> GNU userland (glibc)"
for bin in bash ls cat cp mv rm mkdir rmdir ln touch chmod chown sleep date \
           uname head tail wc sort cut tr env id whoami seq tee basename \
           dirname readlink stat df du sync mktemp hostname grep sed gawk \
           tar gzip zcat cpio find ps curl wget setfont lynx xinit xsetroot xset xrdb \
           lsblk sfdisk mkfs.ext4 mkfs.fat blkid mount umount grub-install partprobe sync; do
    install_gnu "$bin"
done
mkdir -p "$ROOT/etc/lynx"
cp /etc/lynx.cfg "$ROOT/etc/lynx/lynx.cfg"
cp /etc/lynx.lss "$ROOT/etc/lynx/lynx.lss"
ln -sf bash "$ROOT/bin/sh"

# Cyrillic console font for Russian MOTD and messages.
if [ -f /usr/share/kbd/consolefonts/Cyr_a8x16.psfu.gz ]; then
    zcat /usr/share/kbd/consolefonts/Cyr_a8x16.psfu.gz > "$ROOT/etc/consolefont.psfu"
fi

echo "==> X11 (Xorg + xterm + dwm)"
for bin in xauth xkbcomp xrdb xmodmap xdpyinfo xprop xinit startx xterm Xorg X; do
    install_gnu "$bin"
done

mkdir -p "$ROOT/usr/libexec" "$ROOT/usr/share/X11" "$ROOT/usr/share/fonts/X11" "$ROOT/var/lib/xkb" "$ROOT/etc/X11" "$ROOT/etc/lynx"
cp -Lf /usr/libexec/Xorg "$ROOT/usr/libexec/Xorg"
copy_libs /usr/libexec/Xorg

# We run the real server directly: the setuid Xorg.wrap refuses to start when
# init has no controlling terminal (it sees "not a console user").
cat > "$ROOT/usr/bin/Xorg" <<'EOF'
#!/bin/sh
exec /usr/libexec/Xorg "$@"
EOF
chmod 755 "$ROOT/usr/bin/Xorg"
ln -sfn Xorg "$ROOT/usr/bin/X"

# dwm is built from the checked-out, configured source tree in ~/dwm.
DWM_SRC="${DWM_SRC:-$HOME/dwm}"
if [ ! -f "$DWM_SRC/dwm.c" ] || [ ! -f "$DWM_SRC/config.h" ]; then
    echo "dwm source with config.h not found at $DWM_SRC (set DWM_SRC=)" >&2
    exit 1
fi
make -C "$DWM_SRC" clean all
install -m 0755 "$DWM_SRC/dwm" "$ROOT/usr/bin/dwm"
copy_libs "$DWM_SRC/dwm"

cp -a /usr/lib/xorg "$ROOT/usr/lib/xorg"
# Xorg's evdev driver needs the xorg-server ABI helper, and the Xorg
# configuration below uses fbdev plus evdev explicitly.
for module in /usr/lib/xorg/modules/drivers/fbdev_drv.so /usr/lib/xorg/modules/input/evdev_drv.so; do
    [ -f "$module" ] || { echo "required Xorg module missing: $module" >&2; exit 1; }
    copy_libs "$module"
done

cp -a /usr/share/xkeyboard-config-2 "$ROOT/usr/share/xkeyboard-config-2"
ln -sfn ../xkeyboard-config-2 "$ROOT/usr/share/X11/xkb"
cp -a /usr/share/fonts/X11/misc "$ROOT/usr/share/fonts/X11/misc"
cp -a /usr/share/terminfo "$ROOT/usr/share/terminfo"
mkdir -p "$ROOT/usr/share/icons/Oxygen_White/cursors"
cp -a /usr/share/icons/Oxygen_White/cursors/. "$ROOT/usr/share/icons/Oxygen_White/cursors/"
mkdir -p "$ROOT/usr/share/icons/default"
cat > "$ROOT/usr/share/icons/default/index.theme" <<'EOF'
[Icon Theme]
Inherits=Oxygen_White
EOF
mkdir -p "$ROOT/etc"
printf '%s\n' "$REPO_URL" > "$ROOT/etc/wpm-repo"

mkdir -p "$ROOT/etc/X11"
cat > "$ROOT/etc/X11/Xresources" <<'EOF'
Xcursor.theme: Oxygen_White
Xcursor.size: 20
XTerm*font: 9x15
EOF

# A couple of real TTF fonts so xterm/fontconfig are happy (Cyrillic included).
mkdir -p "$ROOT/usr/share/fonts/TTF" "$ROOT/usr/share/fonts/X11/encodings"
for font in DejaVuSans.ttf DejaVuSansMono.ttf DejaVuSansMono-Bold.ttf; do
    [ -f "/usr/share/fonts/TTF/$font" ] && cp "/usr/share/fonts/TTF/$font" "$ROOT/usr/share/fonts/TTF/$font"
done
cp -a /usr/share/fonts/X11/encodings/. "$ROOT/usr/share/fonts/X11/encodings/"
mkdir -p "$ROOT/etc/fonts"
cp -a /etc/fonts/fonts.conf "$ROOT/etc/fonts/fonts.conf"
cp -a /etc/fonts/conf.d "$ROOT/etc/fonts/conf.d"
command -v fc-cache >/dev/null 2>&1 && fc-cache -s -y "$ROOT" >/dev/null 2>&1 || true


cat > "$ROOT/etc/X11/Xwrapper.config" <<'EOF'
needs_root_rights = yes
allowed_users = anybody
EOF

cat > "$ROOT/etc/X11/xorg.conf" <<'EOF'
Section "ServerFlags"
    Option "AutoAddDevices" "true"
    Option "AutoEnableDevices" "true"
EndSection

Section "Files"
    FontPath "/usr/share/fonts/X11/misc"
EndSection

Section "Device"
    Identifier "Framebuffer"
    Driver     "fbdev"
    Option     "fbdev" "/dev/fb0"
EndSection

Section "Monitor"
    Identifier  "Monitor0"
    HorizSync   28.0 - 80.0
    VertRefresh 43.0 - 60.0
EndSection

Section "Screen"
    Identifier "Screen0"
    Device     "Framebuffer"
    Monitor    "Monitor0"
    DefaultDepth 24
    SubSection "Display"
        Depth 24
        Modes "1280x800"
    EndSubSection
EndSection

Section "InputDevice"
    Identifier "Keyboard0"
    Driver "evdev"
    Option "Device" "/dev/input/combine-kbd"
    Option "XkbRules" "evdev"
    Option "XkbModel" "pc105"
    Option "XkbLayout" "us,ru"
    Option "XkbOptions" "grp:alt_shift_toggle"
EndSection

Section "InputDevice"
    Identifier "Mouse0"
    Driver "evdev"
    Option "Device" "/dev/input/combine-mouse"
EndSection

Section "ServerLayout"
    Identifier "Layout0"
    Screen "Screen0"
    InputDevice "Keyboard0" "CoreKeyboard"
    InputDevice "Mouse0" "CorePointer"
EndSection
EOF

cat > "$ROOT/usr/bin/combine-x" <<'EOF'
#!/bin/sh
# Start Xorg with a dwm session on VT7.
if [ ! -e /dev/fb0 ]; then
    echo "combine-x: /dev/fb0 not found (framebuffer module missing?)" >&2
    exit 1
fi
for input in /dev/input/combine-kbd /dev/input/combine-mouse; do
    if [ ! -e "$input" ]; then
        echo "combine-x: $input missing; check /var/log/input.log" >&2
        exit 1
    fi
done

mkdir -p /tmp/.X11-unix
chmod 1777 /tmp/.X11-unix
rm -f /tmp/.X11-unix/X0

/usr/bin/Xorg :0 -ac -nolisten tcp vt7 >/var/log/xorg.log 2>&1 &
xpid=$!
trap 'kill "$xpid" 2>/dev/null' EXIT INT TERM

i=0
while [ ! -S /tmp/.X11-unix/X0 ] && [ "$i" -lt 100 ]; do
    kill -0 "$xpid" 2>/dev/null || {
        echo "combine-x: Xorg failed, see /var/log/xorg.log" >&2
        exit 1
    }
    sleep 0.1
    i=$((i + 1))
done

export DISPLAY=:0
export XAUTHORITY=/dev/null
busybox chvt 7 2>/dev/null || true
xrdb -merge /etc/X11/Xresources 2>/dev/null || true
xsetroot -cursor_name left_ptr 2>/dev/null || true
xterm -bg black -fg white -fa 'DejaVu Sans Mono' -fs 12 -title "COMBINE" -e /bin/bash -l &
child=$!
dwm
kill "$child" 2>/dev/null
kill "$xpid" 2>/dev/null
wait "$xpid" 2>/dev/null
trap - EXIT INT TERM
EOF
chmod 755 "$ROOT/usr/bin/combine-x"

cat > "$ROOT/etc/X11/xinitrc" <<'EOF'
#!/bin/sh
xterm -bg black -fg white -fa 'DejaVu Sans Mono' -fs 12 -title "Login" -e /bin/bash -l
exec dwm
EOF
chmod 755 "$ROOT/etc/X11/xinitrc"

cat > "$ROOT/usr/bin/combine-wm" <<'EOF'
#!/bin/sh
if [ ! -e /dev/fb0 ]; then
    echo "combine-wm: no framebuffer" >&2
    exit 1
fi
export DISPLAY=:0
exec xinit /etc/X11/xinitrc -- :0
EOF
chmod 755 "$ROOT/usr/bin/combine-wm"

# Нативный сервис oneinit: D## — долгоживущий (Xorg + dwm), запускается
# только на 5-м уровне; K## — корректный останов графической сессии.
cat > "$ROOT/etc/init.d/D50-x11" <<'EOF'
#!/bin/sh
exec /usr/bin/combine-x
EOF
chmod 755 "$ROOT/etc/init.d/D50-x11"

cat > "$ROOT/etc/init.d/K50-x11" <<'EOF'
#!/bin/sh
[ "${1:-stop}" = "stop" ] || exit 0
busybox pkill -TERM dwm 2>/dev/null || true
busybox pkill -TERM xterm 2>/dev/null || true
busybox pkill -TERM Xorg 2>/dev/null || true
sleep 0.3
EOF
chmod 755 "$ROOT/etc/init.d/K50-x11"

printf '5\n' > "$ROOT/etc/init.d/x11.levels"

cat > "$ROOT/etc/init.d/S15-input" <<'EOF'
#!/bin/sh
[ "${1:-start}" = "start" ] || exit 0
mkdir -p /dev/input

# Map kernel input handlers to stable absolute device paths. Do not invent
# event0/event1 fallbacks: those can point at unrelated devices or no device.
gawk '
/^I: Bus=/ { name = "" }
/^N: Name=/ { name = tolower($0) }
/^H: Handlers=/ {
    kind = ""
    events = ""
    for (i = 2; i <= NF; i++) {
        if ($i ~ /^event[0-9]+$/) events = events " " $i
        if ($i == "kbd") kind = "kbd"
        if ($i ~ /^mouse[0-9]*$/) kind = "mouse"
    }
    if (kind == "" && name ~ /keyboard|kbd/) { kind = "kbd" }
    if (kind == "" && name ~ /mouse|touchpad|pointer|tablet/) { kind = "mouse" }
    if (kind != "" && events != "") { print kind events }
}' /proc/bus/input/devices 2>/dev/null | while read -r kind events; do
    for ev in $events; do
        case "$kind" in
            kbd)   [ -e "/dev/input/$ev" ] && ln -sf "$ev" /dev/input/combine-kbd ;;
            mouse) [ -e "/dev/input/$ev" ] && ln -sf "$ev" /dev/input/combine-mouse ;;
        esac
    done
done

# Xorg's static evdev configuration is fatal if either selected device is
# absent. Report this early, including the handler list, instead of starting a
# desktop with dead input.
if [ ! -e /dev/input/combine-kbd ]; then
    echo "input: no keyboard event device found; available devices:" >&2
    busybox cat /proc/bus/input/devices >&2
fi
if [ ! -e /dev/input/combine-mouse ]; then
    echo "input: no mouse event device found; available devices:" >&2
    busybox cat /proc/bus/input/devices >&2
fi
exit 0
EOF
chmod 755 "$ROOT/etc/init.d/S15-input"

echo "==> COMBINE tools (wpm)"
for tool in wpm wpm-repo; do
    if [ -x "$PROJECT/target/release/$tool" ]; then
        cp -Lf "$PROJECT/target/release/$tool" "$ROOT/usr/bin/$tool"
        chmod 755 "$ROOT/usr/bin/$tool"
        copy_libs "$PROJECT/target/release/$tool"
    fi
done

cat > "$ROOT/usr/bin/combine-info" <<'EOF'
#!/bin/sh
printf '\n\033[1;36m  COMBINE Linux\033[0m\n'
printf '  kernel    : %s\n' "$(uname -r)"
printf '  shell     : %s\n' "$(bash --version | head -n1)"
printf '  coreutils : %s\n' "$(ls --version | head -n1)"
printf '  wpm       : %s\n' "$(wpm --version 2>/dev/null || echo 'not installed')"
printf '  browser   : %s\n' "$(lynx -version 2>/dev/null | head -n1 || echo 'not installed')"
printf '  x11       : %s\n' "$(/usr/bin/Xorg -version 2>&1 | sed -n 's/^X.Org X Server /Xorg /p')"
printf '  runlevel  : %s\n' "$(sed -n 's/^Current Runlevel: //p' /run/init_status 2>/dev/null)"
printf '  services  : %s\n' "$(in-service list 2>/dev/null | grep -c active || true)"
printf '  uptime    : %s sec\n' "$(cut -d. -f1 /proc/uptime)"
printf '\n'
EOF
chmod 755 "$ROOT/usr/bin/combine-info"

echo "==> system files"
install -m 0644 "$PROJECT/distro/os-release" "$ROOT/etc/os-release"
printf 'combine\n' > "$ROOT/etc/hostname"
mkdir -p "$ROOT/usr/lib/wpm/packages"
mkdir -p "$ROOT/usr/lib/wpm" "$ROOT/usr/lib/wpm/packages"
printf 'COMBINE Linux \\r (\\l)\n\n' > "$ROOT/etc/issue"

cat > "$ROOT/etc/passwd" <<'EOF'
root::0:0:COMBINE root:/root:/bin/bash
EOF
cat > "$ROOT/etc/group" <<'EOF'
root:x:0:
tty:x:5:
EOF
printf 'root::0:0:99999:7:::\n' > "$ROOT/etc/shadow"
chmod 600 "$ROOT/etc/shadow"

cat > "$ROOT/etc/fstab" <<'EOF'
proc      /proc proc     defaults 0 0
sysfs     /sys  sysfs    defaults 0 0
devtmpfs  /dev  devtmpfs defaults 0 0
tmpfs     /run  tmpfs    defaults 0 0
EOF

cat > "$ROOT/etc/init.d/S05-console" <<'EOF'
#!/bin/sh
[ "${1:-start}" = "start" ] || exit 0
[ -f /etc/consolefont.psfu ] && setfont /etc/consolefont.psfu 2>/dev/null
printf '\033%%G'
exit 0
EOF
chmod 755 "$ROOT/etc/init.d/S05-console"

cat > "$ROOT/etc/init.d/S10-combine" <<'EOF'
#!/bin/sh
echo
echo "  COMBINE Linux"
echo "  kernel:    $(uname -r)"
echo "  shell:     $(/bin/bash --version | head -n1)"
echo "  coreutils: $(/bin/ls --version | head -n1)"
echo "  loader:    $(ls /lib64/ld-linux-x86-64.so.2 2>/dev/null)"
echo "  wpm:       $(/usr/bin/wpm --version 2>/dev/null || echo 'not installed')"
echo "  browser:   $(/usr/bin/lynx -version 2>&1 | head -n1)"
echo "  x11:       $(/usr/bin/Xorg -version 2>&1 | sed -n 's/^X.Org X Server /Xorg /p')"
echo
exit 0
EOF
chmod 755 "$ROOT/etc/init.d/S10-combine"

cat > "$ROOT/etc/motd" <<'EOF'

  C O M B I N E   L i n u x
  glibc + GNU coreutils

  combine-info   показать систему
  wpm update     обновить индекс пакетов
  ip addr        сеть (DHCP поднимается автоматически)
  curl / wget    проверить интернет
  lynx <url>     браузер в консоли
  X11            стартует сам при загрузке (runlevel 5, VT7)
  combine-x      перезапустить графику (dwm + xterm)
  combine-wm     X11 через xinit (dwm)
  in-service     список/старт/стоп сервисов oneinit
  telinit 3|5    консоль <-> графика без перезагрузки

EOF

cat > "$ROOT/etc/profile" <<'EOF'
export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
export LANG=C.UTF-8
export LC_ALL=C.UTF-8
export TERM=linux
[ -f /etc/bashrc ] && . /etc/bashrc
[ -x /usr/bin/combine-info ] && /usr/bin/combine-info
EOF
chmod 644 "$ROOT/etc/profile"

cat > "$ROOT/etc/bashrc" <<'EOF'
PS1='\[\e[1;36m\]\u@\h\[\e[0m\]:\[\e[1;34m\]\w\[\e[0m\]\$ '
HISTSIZE=2000
HISTFILE=~/.bash_history
shopt -s histappend checkwinsize
alias ls='ls --color=auto'
alias ll='ls -alF'
alias grep='grep --color=auto'
export PS1
EOF
chmod 644 "$ROOT/etc/bashrc"

cat > "$ROOT/etc/inputrc" <<'EOF'
"\e[A": history-search-backward
"\e[B": history-search-forward
set show-all-if-ambiguous on
set completion-ignore-case on
EOF
chmod 644 "$ROOT/etc/inputrc"

mkdir -p "$ROOT/root" "$ROOT/etc/skel"
printf '[ -f /etc/bashrc ] && . /etc/bashrc\n' > "$ROOT/root/.bashrc"
cp "$ROOT/root/.bashrc" "$ROOT/etc/skel/.bashrc"
chmod 644 "$ROOT/root/.bashrc" "$ROOT/etc/skel/.bashrc"

echo "==> kernel modules (framebuffer + network)"
KVER="$(basename "$KERNEL" | sed 's/^vmlinuz-//')"
MODDIR="$ROOT/lib/modules/$KVER"
mkdir -p "$MODDIR"
for kmod in bochs evdev psmouse virtio_pci virtio_net e1000; do
    deps="$(modprobe --show-depends -S "$KVER" "$kmod" 2>/dev/null || true)"
    printf '%s\n' "$deps" | awk '/^insmod/{print $2}' | while read -r module; do
        rel="${module#/lib/modules/$KVER/}"
        rel="${rel%.zst}"
        mkdir -p "$MODDIR/$(dirname "$rel")"
        case "$module" in
            *.zst) unzstd -c "$module" > "$MODDIR/$rel" ;;
            *)     cp -Lf "$module" "$MODDIR/$rel" ;;
        esac
    done
done
depmod -b "$ROOT" "$KVER" 2>/dev/null || true

echo "==> network + TLS"
mkdir -p "$ROOT/etc/ssl/certs" "$ROOT/usr/share/udhcpc"
if [ -f /etc/ssl/certs/ca-certificates.crt ]; then
    cp /etc/ssl/certs/ca-certificates.crt "$ROOT/etc/ssl/certs/ca-certificates.crt"
fi
printf 'nameserver 10.0.2.3\nnameserver 1.1.1.1\n' > "$ROOT/etc/resolv.conf"

cat > "$ROOT/usr/share/udhcpc/default.script" <<'EOF'
#!/bin/sh
case "$1" in
    deconfig)
        busybox ip addr flush dev "$interface" 2>/dev/null
        ;;
    bound|renew)
        busybox ip addr flush dev "$interface" 2>/dev/null
        busybox ip addr add "$ip/${mask:-24}" dev "$interface"
        if [ -n "$router" ]; then
            busybox ip route add default via "$router" 2>/dev/null
        fi
        : > /etc/resolv.conf
        for d in $dns; do echo "nameserver $d" >> /etc/resolv.conf; done
        ;;
esac
EOF
chmod 755 "$ROOT/usr/share/udhcpc/default.script"

cat > "$ROOT/etc/init.d/S20-network" <<'EOF'
#!/bin/sh
[ "${1:-start}" = "start" ] || exit 0

busybox ip link set lo up 2>/dev/null

iface=""
for _ in $(busybox seq 1 20); do
    for path in /sys/class/net/*; do
        name="${path##*/}"
        [ "$name" = "lo" ] && continue
        iface="$name"
        break
    done
    [ -n "$iface" ] && break
    sleep 0.2
done

if [ -z "$iface" ]; then
    echo "network: no interface found"
    exit 0
fi

busybox ip link set "$iface" up
busybox udhcpc -i "$iface" -q -n -t 5 -T 3 -s /usr/share/udhcpc/default.script >/dev/null 2>&1
addr="$(busybox ip -4 addr show "$iface" 2>/dev/null | sed -n 's/.*inet \([^ ]*\).*/\1/p' | head -n1)"
echo "network: $iface ${addr:-not configured} (dhcp)"
[ -s /etc/resolv.conf ] || printf 'nameserver 10.0.2.3\nnameserver 1.1.1.1\n' > /etc/resolv.conf

if busybox ping -c1 -W2 10.0.2.2 >/dev/null 2>&1; then
    echo "network: gateway reachable"
fi
# Prefer HTTPS via curl (CA bundle is shipped); fall back to plain HTTP.
if curl -fsS -m 8 -o /dev/null https://raw.githubusercontent.com/pristochelovek097/wpm-repo/main/pool/main/busybox 2>/dev/null \
   || busybox wget -q -T 8 -O /dev/null http://example.com 2>/dev/null; then
    echo "network: internet OK"
else
    echo "network: internet unavailable"
fi
exit 0
EOF
chmod 755 "$ROOT/etc/init.d/S20-network"

echo "==> locale (C.UTF-8)"
mkdir -p "$ROOT/usr/lib/locale"
if command -v localedef >/dev/null 2>&1; then
    localedef -c -i C -f UTF-8 "$ROOT/usr/lib/locale/C.utf8" 2>/dev/null || true
fi
if [ ! -f "$ROOT/usr/lib/locale/C.utf8/LC_CTYPE" ] && [ -f /usr/lib/locale/locale-archive ]; then
    cp /usr/lib/locale/locale-archive "$ROOT/usr/lib/locale/locale-archive"
fi

echo "==> installer files"
install -m 0755 "$PROJECT/distro/install.sh" "$ROOT/usr/bin/combine-install"
# Installer marker is set by live init; mount the image's own root for install.

 echo "==> initramfs"
# Make a second copy without a recursive self-image so the installer can
# extract the live root into the target filesystem.
cp -a "$ROOT" "$OUT/install-root"
( cd "$OUT/install-root" && rm -f run/live-root.cpio.gz && find . -print0 | cpio --null -o -H newc 2>/dev/null | gzip -9 ) \
    > "$OUT/live-root.cpio.gz"
install -m 0644 "$OUT/live-root.cpio.gz" "$ROOT/run/live-root.cpio.gz"
( cd "$ROOT" && find . -print0 | cpio --null -o -H newc 2>/dev/null | gzip -9 ) \
    > "$OUT/initrd.img"
rm -rf "$OUT/install-root"

echo "==> ISO"
mkdir -p "$OUT/iso/boot/grub"
cp "$KERNEL" "$OUT/iso/boot/vmlinuz"
cp "$OUT/initrd.img" "$OUT/iso/boot/initrd.img"
cat > "$OUT/iso/boot/grub/grub.cfg" <<'EOF'
set timeout=3
set default=0
menuentry "COMBINE Linux" {
    linux /boot/vmlinuz console=tty0 quiet video=1280x800 runlevel=5
    initrd /boot/initrd.img
}
menuentry "COMBINE Linux (console only)" {
    linux /boot/vmlinuz console=tty0 quiet video=1280x800 runlevel=3
    initrd /boot/initrd.img
}
EOF
grub-mkrescue -o "$OUT/combine.iso" "$OUT/iso" >/dev/null

ls -lh "$OUT/combine.iso" "$OUT/initrd.img" "$ROOT/init"
