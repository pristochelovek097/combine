# COMBINE

> Репозиторий раньше назывался `wilix`. Переименован в `combine`; старое
> содержимое заменено исходниками сборки COMBINE.

Дистрибутив Linux с именем **COMBINE**.
Профиль: **glibc + GNU coreutils** (не musl/BusyBox).

Репозиторий содержит инструменты дистрибутива:

- ядро — отдельный проект, остаётся на C;
- `init/oneinit.c` — адаптированный под COMBINE `oneinit` (источник:
  https://github.com/pristochelovek097/oneinit) со встроенным менеджером
  сервисов и автозапуском графики;
- **wpm** (пакетный менеджер) и **wpm-repo** (генератор индекса репозитория)
  переписаны на Rust.

## Сборка

```bash
git clone https://github.com/pristochelovek097/combine.git
cd combine
cargo build --release
# target/release/wpm
# target/release/wpm-repo
```

Внешних crate-зависимостей нет: сборка работает офлайн, нужен только Rust.
Никаких `openssl-sys`, `ring` и прочего C-линкуемого шума.

## wpm

```bash
sudo wpm update              # обновить индекс Packages
sudo wpm add nano            # установить (с зависимостями из Depends:)
sudo wpm remove nano
wpm list                     # сырой индекс
wpm list -i                  # установленные пакеты (без root)
wpm search foo
wpm info nano
sudo wpm verify              # проверить, что файлы пакетов на месте
sudo wpm clean               # очистить кэш загрузок
```

Ключевое для сборки образов:

```bash
wpm --root ./rootfs --repo https://example.org/repo add bash coreutils
```

`--root` ставит пакеты в указанный rootfs, без root-прав на хосте.

Что изменилось по сравнению со старым C-`wpm`:

- HTTPS через `curl`/`wget`, вся остальная логика — на Rust;
- проверка **MD5** перед установкой (`--no-verify` отключает);
- рекурсивное разрешение `Depends:`, защита от циклов;
- **манифесты файлов**: архивы (`Type: dir`) теперь корректно удаляются и
  проверяются, а не ищутся как `/usr/bin/<имя>`;
- БД переехала в `/var/lib/wpm/installed.list` (старая из
  `/var/cache/wpm/installed.list` читается для совместимости);
- `wpm clean` больше не стирает список установленных пакетов;
- профиль glibc: musl-loader не устанавливается без `--musl`;
- код возврата `10` при запуске без root сохранён.

## wpm-repo

Заменяет bash + Perl из GitHub Action.

```bash
wpm-repo gen   --root /path/to/wpm-repo   # пересобрать блок ###AUTO_START### … ###AUTO_END###
wpm-repo check --root /path/to/wpm-repo   # сверить Size и MD5 всех пакетов
```

Ручной заголовок и подвал `Packages` сохраняются. Мусор `pool/main/lib/a/gcc/`
пропускается, как и раньше. Поддерживаются `.tar.gz`, `.tgz`, `.tar.xz`,
`.txz`, `.tar.bz2`, `.tar`.

Готовый workflow для репозитория пакетов: `contrib/update-packages.yml`.

## Загрузочный ISO

```bash
bash distro/build.sh
# -> /tmp/opencode/combine-build/combine.iso

# Boot live ISO with a disposable 16 GiB QCOW2 virtual disk
bash distro/run-vm.sh
# Disk file: /tmp/opencode/combine-build/combine.qcow2 (no host block device touched)

```

Внутри: COMBINE oneinit (C, статический), glibc, GNU bash 5.x, GNU coreutils,
GNU grep/sed/gawk/find/tar/curl/wget, busybox только как init-хелпер
(getty/login/stty/ip/udhcpc/umount/chvt/pkill), Xorg + dwm + xterm.
Разрешение консоли и X11 — 1280×800 через bochs/fbdev, модули грузятся самим
init'ом. По умолчанию грузится **runlevel 5** — графическая сессия поднимается
сама; пункт `COMBINE Linux (console only)` даёт чистую консоль (runlevel 3).

Сеть: init загружает `virtio_pci` + `virtio_net` (и `e1000`), `S20-network`
поднимает интерфейс и получает адрес по DHCP через `udhcpc`; DNS пишется в
`/etc/resolv.conf`. В QEMU с `-nic user,model=virtio-net-pci` гость получает
`10.0.2.15/24`, шлюз `10.0.2.2`, и интернет через NAT.

Bash: логин-шелл `/bin/bash`, `/etc/profile` + `/etc/bashrc` (цветной
prompt `user@host:dir#`), `/etc/inputrc` с history-search, UTF-8 локаль
`C.UTF-8`, алиасы `ls`/`ll`/`grep`. Кириллица в консоли работает:
`S05-console` грузит шрифт `Cyr_a8x16` и включает UTF-8 режим.

Логин: `root`, пароля нет (live-образ). После входа печатается MOTD и
`combine-info`; команда `combine-info` доступна в любой момент.

Скриншот автоматически поднятой графической сессии: `distro/boot-screenshot.png`.

## Графика (X11)

Образ содержит полноценный X Server, а не только консоль:

- `/usr/libexec/Xorg` + обёртка `/usr/bin/Xorg` (без setuid-враппера,
  который отказывался стартовать из-под init без контролирующего терминала);
- модули `fbdev_drv.so` и `evdev_drv.so`, конфиг `/etc/X11/xorg.conf`
  (1280×800@24, `/dev/fb0`, клавиатура `us,ru` с переключением Alt+Shift);
- `xauth`, `xkbcomp`, `xrdb`, `xmodmap`, `xdpyinfo`, `xprop`, `xinit`, `startx`;
- **dwm** (собирается из `~/dwm`: нужны `dwm.c` и `config.h`) и **xterm**
  с DejaVu Sans Mono;
- шрифты X11/TTF, xkeyboard-config, terminfo, курсоры Oxygen_White, fontconfig.

Команды внутри системы:

```bash
combine-x      # Xorg :0 на VT7 + xterm + dwm (прямой запуск)
combine-wm     # то же через xinit /etc/X11/xinitrc
telinit 3      # уйти в консоль, остановив графику
telinit 5      # вернуть графику
```

Логи: `/var/log/xorg.log` (Xorg) и `/var/log/services.log` (сервисы).

## Сервисы oneinit

`init/oneinit.c` сам управляет сервисами из `/etc/init.d` — это не просто
«прогнать скрипты при загрузке»:

| Имя файла | Роль |
| --- | --- |
| `S##-<name>` | однократный старт (init ждёт завершения) |
| `D##-<name>` | долгоживущий демон (init не ждёт, запоминает pid) |
| `K##-<name>` | останов сервиса |
| `<name>.levels` | цифры уровней, на которых сервис запускается (например, `5`) |

По умолчанию сервис живёт на уровнях 3 и 5. `D50-x11` имеет `x11.levels = 5`,
поэтому X11 стартует только в графическом режиме. При смене уровня `telinit`
останавливает сервисы, которые на новом уровне не нужны, и запускает новые.
Pid-файлы лежат в `/run/<name>.pid`, статус (с кодом возврата последнего
запуска) — в `/run/init_status`.

```bash
in-service list              # список сервисов и их состояние
in-service stop x11          # остановить графику
in-service start x11         # запустить заново
in-service restart x11
in-status                    # общий статус init, включая сервисы
```

## init

`init/oneinit.c` — это upstream `oneinit` на C, адаптированный под COMBINE:
исправлена опечатка `setup_devpts_for` → `setup_devpts`, заменён брендинг
`wilix` → `COMBINE` / hostname `combine`, добавлен нативный менеджер сервисов
(см. выше) и поддержка runlevel 5. Утилиты `telinit`, `in-status` и
`in-service` — симлинки на тот же `/sbin/init`.

## Структура

```
crates/core/       общие модули: Packages, MD5, файловые утилиты
crates/wpm/        пакетный менеджер
crates/wpm-repo/   генератор/валидатор индекса
init/              oneinit для COMBINE (C) + менеджер сервисов
distro/build.sh    сборка rootfs + ISO (включая X11)
distro/install.sh  установка на виртуальный диск из live-образа
distro/run-vm.sh   загрузка ISO в QEMU с disposable QCOW2
distro/os-release  метаданные COMBINE
distro/combine.iso готовый загрузочный образ
contrib/           workflow для wpm-repo
```

## Ограничения

- Переименование GitHub-репозитория `wilix` → `combine` выполнено в настройках
  GitHub (ветка `main` перезаписана исходниками COMBINE).
- `wpm` по-прежнему использует системный `tar` для распаковки архивов. Это
  осознанно: tar есть в любой базовой системе, а своя реализация архиваторов
  не входит в задачу пакетного менеджера.
- Подписи пакетов (GPG) пока не реализованы — только MD5, как в старом
  формате индекса.
