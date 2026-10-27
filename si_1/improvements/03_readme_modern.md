# Modern README template for CactusOS (si_1 proposal)

```markdown
# CactusOS

[![Build](https://github.com/OWNER/CactusOS/actions/workflows/build.yml/badge.svg)](...)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

A 32-bit hobby operating system written from scratch. Not a Linux clone.

## Features
- Higher-half kernel, multitasking, usermode
- VESA graphical desktop + compositor
- USB (UHCI/OHCI/EHCI), mouse, keyboard
- FAT + ISO9660, installer, live CD

## Quick start
```bash
# requires i686-elf-gcc, nasm, xorriso, qemu
make CactusOS.iso
make qemu
```

## Screenshots
...

## Contributing
See CONTRIBUTING.md

## License
MIT
```
