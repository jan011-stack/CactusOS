#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="$ROOT/target/x86_64-unknown-none/release/hyperos-kernel"

for tool in cargo qemu-system-x86_64 grub-mkrescue grub-file; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        printf 'Brak wymaganej komendy: %s\n' "$tool" >&2
        exit 1
    fi
done

if [[ -f "$HOME/.cargo/env" ]]; then
    source "$HOME/.cargo/env"
fi

export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-C link-arg=-T$ROOT/vm/kernel-qemu.ld -C relocation-model=static -C no-redzone=yes"
cargo build --release --target x86_64-unknown-none --features qemu-smoke --bin hyperos-kernel

if ! grub-file --is-x86-multiboot "$TARGET"; then
    printf '%s\n' 'Kernel nie zawiera poprawnego nagłówka Multiboot.' >&2
    exit 1
fi

temp_dir="$(mktemp -d)"
trap 'rm -rf "$temp_dir"' EXIT
iso_root="$temp_dir/iso"
iso_image="$temp_dir/hyperos-qemu-smoke.iso"
mkdir -p "$iso_root/boot/grub"
cp "$TARGET" "$iso_root/boot/hyperos-kernel"
printf '%s\n' \
    'set timeout=0' \
    'set default=0' \
    'menuentry "HyperOS QEMU smoke" {' \
    '    multiboot /boot/hyperos-kernel' \
    '    boot' \
    '}' > "$iso_root/boot/grub/grub.cfg"
grub-mkrescue -o "$iso_image" "$iso_root" >/dev/null

set +e
qemu-system-x86_64 \
    -machine pc,accel=tcg \
    -cpu qemu64 \
    -m 256M \
    -cdrom "$iso_image" \
    -boot order=d \
    -display none \
    -serial stdio \
    -monitor none \
    -no-reboot \
    -no-shutdown \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04
qemu_status=$?
set -e

if [[ "$qemu_status" != 33 ]]; then
    printf 'QEMU smoke test zakończył się kodem %s (oczekiwano 33).\n' "$qemu_status" >&2
    exit 1
fi

printf '%s\n' 'HyperOS QEMU smoke test zakończony poprawnie.'