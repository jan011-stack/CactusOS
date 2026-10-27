# Improvement 01 — Structured Logging

**Problem:** Mixed print statements, no levels, hard to filter in QEMU/Bochs.

**Proposal:**

```cpp
enum class LogLevel { Trace, Debug, Info, Warn, Error, Fatal };

void log(LogLevel level, const char* subsystem, const char* fmt, ...);
```

- Early boot uses serial only
- After VESA: optional on-screen log buffer
- Compile-time filter via `#define LOG_LEVEL`
- Subsystem tags: `"kernel"`, `"usb"`, `"fs.fat"`, `"gui"` etc.

This matches the original planned feature "Update all code to new log method".
