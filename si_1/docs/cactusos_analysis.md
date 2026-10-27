# Technical Analysis of CactusOS (si_1)

## Architecture Overview

- **Kernel**: Higher-half, 32-bit protected mode
- **Graphics**: VESA VBE, software compositor
- **Storage**: PCI IDE (done) + basic AHCI
- **FS**: ISO9660 (read) + FAT12/16/32 (basic)
- **USB**: UHCI / OHCI / EHCI controllers rewritten; xHCI missing
- **Userspace**: Simple apps linked against custom runtime

## Key Directories (upstream Remco123/CactusOS)

```
kernel/          - core kernel + drivers
apps/            - userspace programs (calc, terminal, compositor, desktop…)
initrd/          - initial ramdisk content
isofiles/        - ISO layout + desktop icons + fonts
docs/            - design notes (XML, ODT)
```

## Observations from 2026 perspective

1. **Toolchain**: Still targets classic i686-elf. Works, but documentation should pin exact versions and provide Docker/Nix flake for reproducibility.
2. **Compositor**: Functional but monolithic. Would benefit from clearer separation of window server protocol vs rendering.
3. **Memory management**: Basic. No NUMA, limited isolation guarantees for modern expectations.
4. **Build system**: Classic Makefiles. Consider Meson or CMake for better IDE and CI integration while keeping simplicity.
5. **Testing**: Almost none. Adding QEMU-based integration tests would dramatically improve safety of changes.

## Recommended immediate patches

See `si_1/improvements/` directory for concrete proposals.
