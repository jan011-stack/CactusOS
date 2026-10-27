# CactusOS Modernization Roadmap (si_1)

## Phase 0 — Foundation (1-2 weeks)
- [ ] Add GitHub Actions: build ISO with i686-elf cross-compiler + smoke-test in QEMU
- [ ] Modernize README with badges, clear build prerequisites, screenshots, license
- [ ] Add CONTRIBUTING.md + CODE_OF_CONDUCT.md
- [ ] Introduce structured logging API (replace ad-hoc prints)
- [ ] Fix known boot-partition detection issue

## Phase 1 — Stability (2-4 weeks)
- [ ] Harden FAT driver (write path, error codes, concurrent access)
- [ ] Improve AHCI / IDE error recovery
- [ ] Better exception / panic handling with stack traces
- [ ] Serial console reliability (COM1 + QEMU)
- [ ] Unit-testable common library (list, string, memory)

## Phase 2 — Usability (1-2 months)
- [ ] File Manager application
- [ ] Improved compositor (z-order, clipping, double-buffering polish)
- [ ] Theme system for GUI controls
- [ ] Better keyboard layout support
- [ ] Network stack skeleton (optional, basic Ethernet)

## Phase 3 — Future (longer term)
- [ ] xHCI support or explicit "USB 2.0 only" policy
- [ ] APIC + SMP bootstrap
- [ ] 64-bit long mode path (or clear 32-bit only stance)
- [ ] Package / app format with proper loader API

## Design Principles (si_1)
1. Keep the "not Linux" identity
2. Prefer correctness and clarity over feature count
3. Every new subsystem must have a clear ownership boundary
4. Documentation is part of the code
5. Reproducible builds are non-negotiable
