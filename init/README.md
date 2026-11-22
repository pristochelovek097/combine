# COMBINE init

Это `oneinit` из проекта pristochelovek097/oneinit, адаптированный под COMBINE.
Язык — **C**, не Rust.

Изменения относительно upstream:

- исправлена опечатка: определение `setup_devpts_for()` переименовано в
  `setup_devpts()` (иначе `gcc -static` не линкует бинарь);
- брендинг `wilix` → `COMBINE` в баннере, статусе и логах;
- hostname по умолчанию `wilix` → `combine`;
- в баннер добавлена строка `C O M B I N E   L i n u x` / `glibc + GNU coreutils`;
- `modprobe` грузит `bochs` (framebuffer 1280×800), `virtio_pci` + `virtio_net`
  (сеть в QEMU) и `e1000`. `virtio_pci` обязателен: без него `virtio_net`
  загружается, но не видит ни одного устройства.

Логика загрузки, runlevel'ы, init.d и работа с busybox не менялись.

Лицензия upstream — GPL-3.0, см. `LICENSE`.

Сборка:

```bash
gcc -static -O2 -s -o oneinit oneinit.c
```
