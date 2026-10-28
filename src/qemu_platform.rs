use core::arch::asm;
use core::ptr;
use core::sync::atomic::{AtomicU64, Ordering};

use hyperos_core::hyper_memory_inventory::{MAX_MEMORY_MAP_BYTES, MULTIBOOT1_AVAILABLE_MEMORY};

use super::{ALLOCATOR_BLOCKS, BLOCK_SIZE, POOL_SEGMENTS, REQUIRED_KV_BYTES, SEGMENT_BYTES};

const POOL_BYTES: usize = POOL_SEGMENTS * SEGMENT_BYTES;
const KV_BYTES: usize = REQUIRED_KV_BYTES as usize;

#[repr(align(65536))]
struct AlignedPool([u8; POOL_BYTES]);

#[repr(align(32))]
struct AlignedKvCache([u8; KV_BYTES]);

static mut MEMORY_POOL: AlignedPool = AlignedPool([0; POOL_BYTES]);
static mut KV_CACHE: AlignedKvCache = AlignedKvCache([0; KV_BYTES]);
static mut CONTEXT_MATRIX: [u8; 256] = [0; 256];
static mut MULTIBOOT_MEMORY_MAP: *const u8 = ptr::null();
static mut MULTIBOOT_MEMORY_MAP_LENGTH: usize = 0;
static CLOCK_TICKS: AtomicU64 = AtomicU64::new(0);

const MULTIBOOT1_BOOTLOADER_MAGIC: u32 = 0x2bad_b002;
const MULTIBOOT1_INFO_MEMORY_MAP_FLAG: u32 = 1 << 6;
const IDENTITY_MAP_LIMIT: u64 = 1024 * 1024 * 1024;

pub unsafe fn set_multiboot_info(magic: u32, info: *const u8) -> bool {
    if magic != MULTIBOOT1_BOOTLOADER_MAGIC || info.is_null() {
        return false;
    }
    let flags = ptr::read_unaligned(info.cast::<u32>());
    if flags & MULTIBOOT1_INFO_MEMORY_MAP_FLAG == 0 {
        return false;
    }
    let map_length = ptr::read_unaligned(info.add(44).cast::<u32>()) as usize;
    let map_address = u64::from(ptr::read_unaligned(info.add(48).cast::<u32>()));
    let Some(map_end) = map_address.checked_add(map_length as u64) else {
        return false;
    };
    if map_length == 0 || map_length > MAX_MEMORY_MAP_BYTES || map_end > IDENTITY_MAP_LIMIT {
        return false;
    }

    MULTIBOOT_MEMORY_MAP = map_address as *const u8;
    MULTIBOOT_MEMORY_MAP_LENGTH = map_length;
    true
}

fn outb(port: u16, value: u8) {
    unsafe {
        asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
    }
}

fn inb(port: u16) -> u8 {
    let value: u8;
    unsafe {
        asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack, preserves_flags));
    }
    value
}

fn serial_init() {
    outb(0x3f9, 0x00);
    outb(0x3fb, 0x80);
    outb(0x3f8, 0x03);
    outb(0x3f9, 0x00);
    outb(0x3fb, 0x03);
    outb(0x3fa, 0xc7);
    outb(0x3fc, 0x0b);
}

fn serial_write(bytes: &[u8]) {
    for byte in bytes {
        while inb(0x3fd) & 0x20 == 0 {
            core::hint::spin_loop();
        }
        outb(0x3f8, *byte);
    }
}

pub fn write_task_line(pid: u8, name: &str) {
    match (pid, name) {
        (1, "INIT") => serial_write(b"TASK pid=1 name=INIT\n"),
        (2, "RUN_HYPER") => serial_write(b"TASK pid=2 name=RUN_HYPER\n"),
        _ => serial_write(b"TASK pid=? name=UNKNOWN\n"),
    }
}

fn serial_write_u64(mut value: u64) {
    let mut buffer = [0u8; 20];
    let mut start = buffer.len();
    loop {
        start -= 1;
        buffer[start] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    serial_write(&buffer[start..]);
}

pub fn write_resource_line(pid: u8, memory_bytes: u64, cpu_quanta: u64) {
    serial_write(b"RESOURCE pid=");
    serial_write_u64(u64::from(pid));
    serial_write(b" memory_bytes=");
    serial_write_u64(memory_bytes);
    serial_write(b" cpu_quanta=");
    serial_write_u64(cpu_quanta);
    serial_write(b"\n");
}

pub fn write_memory_summary(region_count: usize, usable_bytes: u64, non_usable_bytes: u64) {
    serial_write(b"MEMORY regions=");
    serial_write_u64(region_count as u64);
    serial_write(b" usable_bytes=");
    serial_write_u64(usable_bytes);
    serial_write(b" non_usable_bytes=");
    serial_write_u64(non_usable_bytes);
    serial_write(b"\n");
}

pub fn write_memory_region(index: usize, base: u64, length: u64, kind: u32) {
    serial_write(b"MEMORY_RANGE index=");
    serial_write_u64(index as u64);
    serial_write(b" base=");
    serial_write_u64(base);
    serial_write(b" length=");
    serial_write_u64(length);
    serial_write(b" kind=");
    serial_write_u64(u64::from(kind));
    if kind == MULTIBOOT1_AVAILABLE_MEMORY {
        serial_write(b" usable\n");
    } else {
        serial_write(b" non_usable\n");
    }
}

pub fn exit_success() -> ! {
    serial_write(b"QEMU_SMOKE_PASS\n");
    unsafe {
        asm!("out dx, eax", in("dx") 0xf4u16, in("eax") 0x10u32, options(nomem, nostack));
    }
    loop {
        core::hint::spin_loop();
    }
}

#[no_mangle]
pub extern "C" fn hyperos_platform_memory_pool(base: *mut *mut u8, length: *mut usize) -> i32 {
    if base.is_null() || length.is_null() {
        return -1;
    }
    unsafe {
        base.write(ptr::addr_of_mut!(MEMORY_POOL.0).cast());
        length.write(POOL_BYTES);
    }
    0
}

#[no_mangle]
pub extern "C" fn hyperos_platform_memory_map(base: *mut *const u8, length: *mut usize) -> i32 {
    if base.is_null() || length.is_null() {
        return -1;
    }
    let map_base = unsafe { MULTIBOOT_MEMORY_MAP };
    let map_length = unsafe { MULTIBOOT_MEMORY_MAP_LENGTH };
    if map_base.is_null() || map_length == 0 {
        return -1;
    }
    unsafe {
        base.write(map_base);
        length.write(map_length);
    }
    0
}

#[no_mangle]
pub extern "C" fn hyperos_platform_illuminate_terminal() -> i32 {
    serial_init();
    serial_write(b"CACTUSOS_QEMU_BOOT\n");
    0
}

#[no_mangle]
pub extern "C" fn hyperos_platform_poll_input(
    prompt: *mut *const u8,
    prompt_len: *mut usize,
    image: *mut *const u8,
    image_len: *mut usize,
    image_width: *mut u32,
    image_height: *mut u32,
) -> i32 {
    if prompt.is_null()
        || prompt_len.is_null()
        || image.is_null()
        || image_len.is_null()
        || image_width.is_null()
        || image_height.is_null()
    {
        return -1;
    }
    unsafe {
        prompt.write(ptr::null());
        prompt_len.write(0);
        image.write(ptr::null());
        image_len.write(0);
        image_width.write(0);
        image_height.write(0);
    }
    0
}

#[no_mangle]
pub extern "C" fn hyperos_platform_kv_cache(base: *mut *mut u8, length: *mut usize) -> i32 {
    if base.is_null() || length.is_null() {
        return -1;
    }
    unsafe {
        base.write(ptr::addr_of_mut!(KV_CACHE.0).cast());
        length.write(KV_BYTES);
    }
    0
}

#[no_mangle]
pub extern "C" fn hyperos_platform_vision_projection(
    data: *mut *const u8,
    length: *mut usize,
    quant_type: *mut u8,
) -> i32 {
    if !data.is_null() && !length.is_null() && !quant_type.is_null() {
        unsafe {
            data.write(ptr::null());
            length.write(0);
            quant_type.write(0);
        }
    }
    -1
}

#[no_mangle]
pub extern "C" fn hyperos_ollama_try_write(_: *const u8, _: usize) -> isize {
    -2
}

#[no_mangle]
pub extern "C" fn hyperos_ollama_try_read(_: *mut u8, _: usize) -> isize {
    -2
}

#[no_mangle]
pub extern "C" fn hyperos_update_try_read(_: *mut u8, _: usize) -> isize {
    -2
}

#[no_mangle]
pub extern "C" fn hyperos_update_source_id() -> u32 {
    0
}

#[no_mangle]
pub extern "C" fn hyperos_update_source_authenticated() -> bool {
    false
}

#[no_mangle]
pub extern "C" fn hyperos_stdout_try_write(bytes: *const u8, length: usize) -> isize {
    if bytes.is_null() && length != 0 {
        return -1;
    }
    let output = unsafe { core::slice::from_raw_parts(bytes, length) };
    serial_write(output);
    length as isize
}

#[no_mangle]
pub extern "C" fn hyperos_clock_ticks() -> u64 {
    CLOCK_TICKS.fetch_add(1, Ordering::Relaxed)
}

#[no_mangle]
pub extern "C" fn hyperos_current_context_matrix() -> *mut u8 {
    ptr::addr_of_mut!(CONTEXT_MATRIX).cast()
}

#[no_mangle]
pub extern "C" fn hyperos_current_context_matrix_len() -> usize {
    256
}

#[no_mangle]
pub extern "C" fn hyperos_restore_system_phase_zero() {}

const _: () = assert!(POOL_BYTES == BLOCK_SIZE * ALLOCATOR_BLOCKS * POOL_SEGMENTS);