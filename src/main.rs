#![no_std]
#![no_main]

#[cfg(feature = "qemu-smoke")]
mod qemu_platform;

use core::mem::MaybeUninit;
use core::panic::PanicInfo;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering};
use sha2::{Digest, Sha256};

use hyperos_core::chrono_log::{ChronoLogEngine, LogEntry};
use hyperos_core::hyper_allocator::HyperAllocator;
use hyperos_core::hyper_bridge_loop::{HyperBridgeLoop, LoopProgress, NonBlockingOutput};
use hyperos_core::hyper_chrono_guard::HyperChronoGuard;
use hyperos_core::hyper_diagnostics::run_post_validation;
use hyperos_core::hyper_euler_engine::compute_euler_rotation;
use hyperos_core::hyper_memory_inventory::{parse_memory_map, MAX_MEMORY_MAP_BYTES};
use hyperos_core::hyper_model::{GgmlType, GgufTensor, TensorInfo};
use hyperos_core::hyper_ollama_client::{IoError, NonBlockingSocket};
use hyperos_core::hyper_resource_manager::ResourceRequest;
use hyperos_core::hyper_scheduler::{HyperScheduler, TASK_SLOTS};
use hyperos_core::hyper_shield::{HyperShieldController, SecurityRing};
use hyperos_core::hyper_update_engine::{
    AuthenticatedUpdateSource, HyperUpdateEngine, UpdateProgress,
};
use hyperos_core::hyper_vision::HyperVisionEngine;
use hyperos_core::kernel::HyperKernel;
use hyperos_core::sanitized_stream_reader::SanitizedStreamReader;
use hyperos_core::terminal::{HyperSpaceTerminal, VECTOR_BYTES};

#[cfg(feature = "qemu-smoke")]
const BLOCK_SIZE: usize = 64 * 1024;
#[cfg(not(feature = "qemu-smoke"))]
const BLOCK_SIZE: usize = 8 * 1024 * 1024;
#[cfg(feature = "qemu-smoke")]
const POOL_SEGMENTS: usize = 8;
#[cfg(not(feature = "qemu-smoke"))]
const POOL_SEGMENTS: usize = 60;
const ALLOCATOR_BLOCKS: usize = 64;
const SEGMENT_BYTES: usize = BLOCK_SIZE * ALLOCATOR_BLOCKS;
#[cfg(feature = "qemu-smoke")]
const REQUIRED_POOL_BYTES: u64 = (SEGMENT_BYTES * POOL_SEGMENTS) as u64;
#[cfg(not(feature = "qemu-smoke"))]
const REQUIRED_POOL_BYTES: u64 = 30 * 1024 * 1024 * 1024;
#[cfg(feature = "qemu-smoke")]
const REQUIRED_KV_BYTES: u64 = 16 * 1024 * 1024;
#[cfg(not(feature = "qemu-smoke"))]
const REQUIRED_KV_BYTES: u64 = 16 * 1024 * 1024 * 1024;
const RESPONSE_CAPACITY: usize = 4096;
const TASK_LOG_CONTEXT_BASE: u32 = 0x4859_0000;
const RESOURCE_LOG_CONTEXT_BASE: u32 = 0x4859_1000;
const MEMORY_LOG_CONTEXT_BASE: u32 = 0x4859_2000;
const CPU_QUANTA_PER_EPOCH: u64 = 64;

#[cfg(feature = "qemu-smoke")]
core::arch::global_asm!(include_str!("qemu_boot.s"), options(att_syntax));

static mut RESPONSE_BUFFER: [u8; RESPONSE_CAPACITY] = [0; RESPONSE_CAPACITY];
static INIT_TASK_RUNS: AtomicUsize = AtomicUsize::new(0);
static RUN_HYPER_TASK_RUNS: AtomicUsize = AtomicUsize::new(0);
static mut PREBOOT_POOL_BASE: *const u8 = core::ptr::null();
static mut PREBOOT_POOL_LENGTH: usize = 0;
static mut PREBOOT_OPERATOR_BASE: *const u8 = core::ptr::null();
static mut PREBOOT_OPERATOR_LENGTH: usize = 0;

// All functions below are supplied by the target's bootloader/platform layer.
// Input slices remain valid and immutable until the next input poll.
extern "C" {
    fn hyperos_platform_memory_pool(base: *mut *mut u8, length: *mut usize) -> i32;
    // Returns a bounded buffer of Multiboot-v1-compatible physical memory-map entries.
    fn hyperos_platform_memory_map(base: *mut *const u8, length: *mut usize) -> i32;
    fn hyperos_platform_illuminate_terminal() -> i32;
    fn hyperos_platform_poll_input(
        prompt: *mut *const u8,
        prompt_len: *mut usize,
        image: *mut *const u8,
        image_len: *mut usize,
        image_width: *mut u32,
        image_height: *mut u32,
    ) -> i32;
    fn hyperos_platform_kv_cache(base: *mut *mut u8, length: *mut usize) -> i32;
    fn hyperos_platform_vision_projection(
        data: *mut *const u8,
        length: *mut usize,
        quant_type: *mut u8,
    ) -> i32;
    fn hyperos_ollama_try_write(bytes: *const u8, length: usize) -> isize;
    fn hyperos_ollama_try_read(bytes: *mut u8, capacity: usize) -> isize;
    fn hyperos_update_try_read(bytes: *mut u8, capacity: usize) -> isize;
    fn hyperos_update_source_id() -> u32;
    fn hyperos_update_source_authenticated() -> bool;
    fn hyperos_stdout_try_write(bytes: *const u8, length: usize) -> isize;
    fn hyperos_clock_ticks() -> u64;
    fn hyperos_current_context_matrix() -> *mut u8;
    fn hyperos_current_context_matrix_len() -> usize;
    fn hyperos_restore_system_phase_zero();
}

struct OllamaSocket;

impl NonBlockingSocket for OllamaSocket {
    fn try_write(&mut self, bytes: &[u8]) -> Result<usize, IoError> {
        socket_result(unsafe { hyperos_ollama_try_write(bytes.as_ptr(), bytes.len()) })
    }

    fn try_read(&mut self, bytes: &mut [u8]) -> Result<usize, IoError> {
        socket_result(unsafe { hyperos_ollama_try_read(bytes.as_mut_ptr(), bytes.len()) })
    }
}

struct UpdateSocket;

impl NonBlockingSocket for UpdateSocket {
    fn try_write(&mut self, _: &[u8]) -> Result<usize, IoError> {
        Err(IoError::Failure)
    }

    fn try_read(&mut self, bytes: &mut [u8]) -> Result<usize, IoError> {
        socket_result(unsafe { hyperos_update_try_read(bytes.as_mut_ptr(), bytes.len()) })
    }
}

impl AuthenticatedUpdateSource for UpdateSocket {
    fn source_id(&self) -> u32 {
        unsafe { hyperos_update_source_id() }
    }

    fn is_authenticated(&self) -> bool {
        unsafe { hyperos_update_source_authenticated() }
    }
}

struct Stdout;

impl NonBlockingOutput for Stdout {
    fn try_write(&mut self, bytes: &[u8]) -> Result<usize, IoError> {
        socket_result(unsafe { hyperos_stdout_try_write(bytes.as_ptr(), bytes.len()) })
    }
}

fn socket_result(result: isize) -> Result<usize, IoError> {
    match result {
        -2 => Err(IoError::WouldBlock),
        -1 => Err(IoError::Failure),
        0 => Err(IoError::Closed),
        count if count > 0 => Ok(count as usize),
        _ => Err(IoError::Failure),
    }
}

fn init_task() {
    INIT_TASK_RUNS.fetch_add(1, Ordering::Relaxed);
}

fn run_hyper_task() {
    RUN_HYPER_TASK_RUNS.fetch_add(1, Ordering::Relaxed);
}

fn log_event(log: &ChronoLogEngine, entry: LogEntry) {
    if log.push(entry).is_err() {
        let _ = log.pop();
        let _ = log.push(entry);
    }
}

fn log_task_info(log: &ChronoLogEngine, scheduler: &HyperScheduler, slot: usize, timestamp: u64) {
    let Some(info) = scheduler.task_info(slot) else {
        return;
    };
    let mut status_snapshot = [0; VECTOR_BYTES];
    let name = info.name.as_bytes();
    let name_length = name.len().min(VECTOR_BYTES - 11);
    status_snapshot[0] = info.pid;
    status_snapshot[1] = info.slot;
    status_snapshot[2..6].copy_from_slice(&info.weight.to_le_bytes());
    status_snapshot[6..10].copy_from_slice(&info.coordinate.to_le_bytes());
    status_snapshot[10] = name_length as u8;
    status_snapshot[11..11 + name_length].copy_from_slice(&name[..name_length]);
    log_event(
        log,
        LogEntry {
            timestamp,
            context_id: TASK_LOG_CONTEXT_BASE | u32::from(info.pid),
            status_snapshot,
        },
    );
    #[cfg(feature = "qemu-smoke")]
    qemu_platform::write_task_line(info.pid, info.name);
}

pub fn pre_boot_sanity_check() -> bool {
    let (pool, pool_length, operator, operator_length) = unsafe {
        (
            core::ptr::read_volatile(core::ptr::addr_of!(PREBOOT_POOL_BASE)),
            core::ptr::read_volatile(core::ptr::addr_of!(PREBOOT_POOL_LENGTH)),
            core::ptr::read_volatile(core::ptr::addr_of!(PREBOOT_OPERATOR_BASE)),
            core::ptr::read_volatile(core::ptr::addr_of!(PREBOOT_OPERATOR_LENGTH)),
        )
    };
    if pool.is_null() || operator.is_null() || operator_length == 0 {
        return false;
    }

    for offset in 0..pool_length {
        // SAFETY: bootloader supplied a readable pool of the declared length.
        if unsafe { core::ptr::read_volatile(pool.add(offset)) } != 0 {
            return false;
        }
    }

    let actual_operator = unsafe { core::slice::from_raw_parts(operator, operator_length) };
    let actual_hash = Sha256::digest(actual_operator);
    let expected_hash = Sha256::digest(b"Mateusz Mitek");
    let mut difference = 0u8;
    for index in 0..VECTOR_BYTES {
        difference |= actual_hash[index] ^ expected_hash[index];
    }
    difference == 0
}

#[cfg(not(feature = "qemu-smoke"))]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    kernel_entry()
}

#[cfg(feature = "qemu-smoke")]
#[no_mangle]
pub extern "C" fn hyperos_kernel_main(magic: u32, info: *const u8) -> ! {
    if !unsafe { qemu_platform::set_multiboot_info(magic, info) } {
        panic!("HyperOS: bootloader did not provide a valid Multiboot memory map");
    }
    kernel_entry()
}

fn kernel_entry() -> ! {
    let mut memory_map_base = core::ptr::null();
    let mut memory_map_length = 0usize;
    if unsafe { hyperos_platform_memory_map(&mut memory_map_base, &mut memory_map_length) } != 0
        || memory_map_base.is_null()
        || memory_map_length > MAX_MEMORY_MAP_BYTES
    {
        panic!("HyperOS: platform did not provide a valid physical memory map");
    }
    let memory_map_bytes = unsafe {
        core::slice::from_raw_parts(memory_map_base, memory_map_length)
    };
    let memory_inventory = parse_memory_map(memory_map_bytes)
        .unwrap_or_else(|_| panic!("HyperOS: physical memory map is malformed"));

    let mut pool_base = core::ptr::null_mut();
    let mut pool_length = 0usize;
    if unsafe { hyperos_platform_memory_pool(&mut pool_base, &mut pool_length) } != 0 {
        panic!("HyperOS: platform did not provide the static memory pool");
    }
    let required = usize::try_from(REQUIRED_POOL_BYTES)
        .unwrap_or_else(|_| panic!("HyperOS requires a 64-bit target"));
    let Some(pool) = NonNull::new(pool_base) else {
        panic!("HyperOS: null memory pool");
    };
    if pool_length < required || (pool.as_ptr() as usize) & (BLOCK_SIZE - 1) != 0 {
        panic!("HyperOS: memory pool is undersized or misaligned");
    }
    if !memory_inventory.contains_usable_range(pool.as_ptr() as u64, required as u64) {
        panic!("HyperOS: allocator pool is outside usable physical memory");
    }

    let kernel = HyperKernel::new();
    unsafe {
        core::ptr::write_volatile(core::ptr::addr_of_mut!(PREBOOT_POOL_BASE), pool.as_ptr());
        core::ptr::write_volatile(core::ptr::addr_of_mut!(PREBOOT_POOL_LENGTH), required);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!(PREBOOT_OPERATOR_BASE),
            kernel.operator().as_ptr(),
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!(PREBOOT_OPERATOR_LENGTH),
            kernel.operator().len(),
        );
    }
    if !run_post_validation() || !pre_boot_sanity_check() {
        panic!("HyperOS pre-boot integrity validation failed");
    }
    if unsafe { hyperos_platform_illuminate_terminal() } != 0 {
        panic!("HyperOS: terminal display initialization failed");
    }
    #[cfg(feature = "qemu-smoke")]
    {
        qemu_platform::write_memory_summary(
            memory_inventory.region_count(),
            memory_inventory.usable_bytes(),
            memory_inventory.non_usable_bytes(),
        );
        for index in 0..memory_inventory.region_count() {
            let region = memory_inventory.region(index).unwrap();
            qemu_platform::write_memory_region(index, region.base, region.length, region.kind);
        }
    }

    let mut allocators: [MaybeUninit<HyperAllocator<BLOCK_SIZE>>; POOL_SEGMENTS] =
        [const { MaybeUninit::uninit() }; POOL_SEGMENTS];
    for (index, slot) in allocators.iter_mut().enumerate() {
        let address = unsafe { pool.as_ptr().add(index * SEGMENT_BYTES) };
        let segment = NonNull::new(address).unwrap_or_else(|| panic!("HyperOS: invalid pool segment"));
        let allocator = unsafe { HyperAllocator::<BLOCK_SIZE>::new(segment, SEGMENT_BYTES) }
            .unwrap_or_else(|_| panic!("HyperOS: allocator initialization failed"));
        slot.write(allocator);
    }

    let mut kv_base = core::ptr::null_mut();
    let mut kv_length = 0usize;
    if unsafe { hyperos_platform_kv_cache(&mut kv_base, &mut kv_length) } != 0
        || u64::try_from(kv_length).unwrap_or(0) < REQUIRED_KV_BYTES
        || kv_base.is_null()
        || (kv_base as usize) & (VECTOR_BYTES - 1) != 0
    {
        panic!("HyperOS: platform did not provide the aligned 16 GiB KV cache");
    }
    if !memory_inventory.contains_usable_range(kv_base as u64, kv_length as u64) {
        panic!("HyperOS: KV cache is outside usable physical memory");
    }
    let kv_entry_count = kv_length / VECTOR_BYTES;
    let kv_cache: &'static mut [[u8; VECTOR_BYTES]] = unsafe {
        core::slice::from_raw_parts_mut(kv_base.cast(), kv_entry_count)
    };

    let terminal = HyperSpaceTerminal::new();
    let mut scheduler = HyperScheduler::new();
    if scheduler.register_named(0, "INIT", init_task).is_err()
        || scheduler
            .register_named(1, "RUN_HYPER", run_hyper_task)
            .is_err()
    {
        panic!("HyperOS: scheduler initialization failed");
    }

    let chat_log = ChronoLogEngine::new();
    let mut memory_summary = [0; VECTOR_BYTES];
    memory_summary[0..8].copy_from_slice(&memory_inventory.usable_bytes().to_le_bytes());
    memory_summary[8..16].copy_from_slice(&memory_inventory.non_usable_bytes().to_le_bytes());
    memory_summary[16..20]
        .copy_from_slice(&(memory_inventory.region_count() as u32).to_le_bytes());
    log_event(
        &chat_log,
        LogEntry {
            timestamp: unsafe { hyperos_clock_ticks() },
            context_id: MEMORY_LOG_CONTEXT_BASE,
            status_snapshot: memory_summary,
        },
    );
    for index in 0..memory_inventory.region_count() {
        let region = memory_inventory.region(index).unwrap();
        let mut status_snapshot = [0; VECTOR_BYTES];
        status_snapshot[0..8].copy_from_slice(&region.base.to_le_bytes());
        status_snapshot[8..16].copy_from_slice(&region.length.to_le_bytes());
        status_snapshot[16..20].copy_from_slice(&region.kind.to_le_bytes());
        status_snapshot[20..24].copy_from_slice(&(index as u32).to_le_bytes());
        log_event(
            &chat_log,
            LogEntry {
                timestamp: unsafe { hyperos_clock_ticks() },
                context_id: MEMORY_LOG_CONTEXT_BASE | (index as u32 + 1),
                status_snapshot,
            },
        );
    }
    let memory_budget = (POOL_SEGMENTS * ALLOCATOR_BLOCKS) as u64;
    let inactive_request = ResourceRequest {
        minimum: 0,
        demand: 0,
        maximum: 0,
    };
    let mut resource_requests = [inactive_request; TASK_SLOTS];
    for (slot, request) in resource_requests.iter_mut().enumerate() {
        if scheduler.task_info(slot).is_some() {
            *request = ResourceRequest {
                minimum: 0,
                demand: 0,
                maximum: memory_budget.max(CPU_QUANTA_PER_EPOCH),
            };
        }
    }
    let mut memory_quotas = [0u64; TASK_SLOTS];
    let memory_plan = scheduler
        .resource_quotas(&resource_requests, memory_budget, 0, &mut memory_quotas)
        .unwrap_or_else(|_| panic!("HyperOS: memory quota calculation failed"));
    let mut cpu_quotas = [0u64; TASK_SLOTS];
    let cpu_plan = scheduler
        .resource_quotas(
            &resource_requests,
            CPU_QUANTA_PER_EPOCH,
            0,
            &mut cpu_quotas,
        )
        .unwrap_or_else(|_| panic!("HyperOS: CPU quota calculation failed"));
    for slot in 0..TASK_SLOTS {
        let Some(info) = scheduler.task_info(slot) else {
            continue;
        };
        let mut status_snapshot = [0; VECTOR_BYTES];
        status_snapshot[0] = info.pid;
        status_snapshot[1..9].copy_from_slice(&(memory_quotas[slot] * BLOCK_SIZE as u64).to_le_bytes());
        status_snapshot[9..17].copy_from_slice(&cpu_quotas[slot].to_le_bytes());
        log_event(
            &chat_log,
            LogEntry {
                timestamp: unsafe { hyperos_clock_ticks() },
                context_id: RESOURCE_LOG_CONTEXT_BASE | u32::from(info.pid),
                status_snapshot,
            },
        );
        #[cfg(feature = "qemu-smoke")]
        qemu_platform::write_resource_line(
            info.pid,
            memory_quotas[slot] * BLOCK_SIZE as u64,
            cpu_quotas[slot],
        );
    }
    if memory_plan.unused != 0 || cpu_plan.unused != 0 {
        panic!("HyperOS: resource quota plan left budget unused");
    }

    let shield = HyperShieldController::new();
    let mut update_engine = HyperUpdateEngine::new();
    let mut update_guard = HyperChronoGuard::new();
    let vision_engine = HyperVisionEngine::new();
    let mut request_loop = HyperBridgeLoop::new();

    let response_storage = unsafe {
        core::slice::from_raw_parts_mut(
            core::ptr::addr_of_mut!(RESPONSE_BUFFER).cast::<u8>(),
            RESPONSE_CAPACITY,
        )
    };
    let mut response_reader = SanitizedStreamReader::new(response_storage);
    let mut ollama_socket = OllamaSocket;
    let mut update_socket = UpdateSocket;
    let mut stdout = Stdout;
    let mut shadow = [0u8; 256];
    let mut snapshot = LogEntry {
        timestamp: 0,
        context_id: 0,
        status_snapshot: [0; VECTOR_BYTES],
    };
    let mut request_active = false;
    let mut kv_cursor = 0usize;
    let mut init_task_logged = false;
    let mut run_hyper_task_logged = false;

    loop {
        let ticks = unsafe { hyperos_clock_ticks() };

        let phase = (ticks & 7) as f64;
        let _ = scheduler.run_slice(phase);
        if !init_task_logged && INIT_TASK_RUNS.load(Ordering::Relaxed) != 0 {
            log_task_info(&chat_log, &scheduler, 0, ticks);
            init_task_logged = true;
        }
        if !run_hyper_task_logged && RUN_HYPER_TASK_RUNS.load(Ordering::Relaxed) != 0 {
            log_task_info(&chat_log, &scheduler, 1, ticks);
            run_hyper_task_logged = true;
        }
        #[cfg(feature = "qemu-smoke")]
        if init_task_logged && run_hyper_task_logged {
            qemu_platform::exit_success();
        }

        match update_engine.poll(
            &mut update_socket,
            &terminal,
            &mut update_guard,
            None,
            &snapshot,
            &mut shadow,
        ) {
            Ok(UpdateProgress::Staged { feature_delta, .. }) => {
                snapshot.status_snapshot = feature_delta;
                log_event(&chat_log, snapshot);
            }
            Ok(UpdateProgress::Pending) | Err(_) => {}
        }

        if !request_active {
            let mut prompt_pointer = core::ptr::null();
            let mut prompt_length = 0usize;
            let mut image_pointer = core::ptr::null();
            let mut image_length = 0usize;
            let mut image_width = 0u32;
            let mut image_height = 0u32;
            let input_ready = unsafe {
                hyperos_platform_poll_input(
                    &mut prompt_pointer,
                    &mut prompt_length,
                    &mut image_pointer,
                    &mut image_length,
                    &mut image_width,
                    &mut image_height,
                )
            };
            if input_ready > 0 && prompt_length <= 256 {
                let prompt: &'static [u8] = if prompt_length == 0 {
                    &[]
                } else if !prompt_pointer.is_null() {
                    unsafe { core::slice::from_raw_parts(prompt_pointer, prompt_length) }
                } else {
                    &[]
                };
                let image: &'static [u8] = if image_length == 0 {
                    &[]
                } else if !image_pointer.is_null() {
                    unsafe { core::slice::from_raw_parts(image_pointer, image_length) }
                } else {
                    &[]
                };
                if (prompt_length == 0 || !prompt_pointer.is_null())
                    && (image_length == 0 || !image_pointer.is_null())
                {
                    response_reader.reset();
                    if request_loop
                        .begin(&terminal, prompt, image)
                        .is_ok()
                    {
                        snapshot.timestamp = ticks;
                        snapshot.context_id = 0;
                        let prompt_text = core::str::from_utf8(prompt).unwrap_or("");
                        let vector_ok = terminal.process_text_command(
                            prompt_text,
                            &mut snapshot.status_snapshot,
                        );
                        if image_length != 0 {
                            let mut projection_pointer = core::ptr::null();
                            let mut projection_length = 0usize;
                            let mut quant_type = 0u8;
                            if unsafe {
                                hyperos_platform_vision_projection(
                                    &mut projection_pointer,
                                    &mut projection_length,
                                    &mut quant_type,
                                )
                            } == 0
                                && !projection_pointer.is_null()
                            {
                                let projection_data: &'static [u8] = unsafe {
                                    core::slice::from_raw_parts(
                                        projection_pointer,
                                        projection_length,
                                    )
                                };
                                let tensor_type = match quant_type {
                                    4 => Some(GgmlType::Q4_0),
                                    8 => Some(GgmlType::Q8_0),
                                    _ => None,
                                };
                                if let Some(tensor_type) = tensor_type {
                                    let tensor = GgufTensor {
                                        info: TensorInfo {
                                            name: "vision.projection",
                                            dimensions: [32, 32, 1, 1],
                                            dimension_count: 2,
                                            tensor_type,
                                            relative_offset: 0,
                                            byte_length: projection_data.len(),
                                        },
                                        data: projection_data,
                                    };
                                    if kv_cursor == kv_cache.len() {
                                        kv_cursor = 0;
                                    }
                                    let _ = vision_engine.process_step(
                                        &snapshot.status_snapshot,
                                        image,
                                        image_width,
                                        image_height,
                                        &tensor,
                                        phase,
                                        kv_cache,
                                        &mut kv_cursor,
                                    );
                                }
                            }
                        }
                        // Ring-0 boundary check of the command vector.
                        let boundary_ok = vector_ok
                            && terminal.validate_orthogonal(&snapshot.status_snapshot);
                        log_event(&chat_log, snapshot);
                        // Euler phase transform of the current tick.
                        let (real, imag) = compute_euler_rotation(phase);
                        let mut continuum = snapshot;
                        continuum.status_snapshot[0] = real as i8 as u8;
                        continuum.status_snapshot[1] = imag as i8 as u8;
                        log_event(&chat_log, continuum);
                        request_active = boundary_ok;
                    }
                }
            }
        }

        if request_active {
            match request_loop.poll(&mut ollama_socket, &mut stdout, &mut response_reader) {
                Ok(LoopProgress::Complete) => request_active = false,
                Ok(LoopProgress::Pending) => {}
                Ok(LoopProgress::StreamWarning(flags)) => {
                    let warning = [b'!', b' ', b'0' + (flags & 0x0f), b'\n'];
                    let _ = stdout.try_write(&warning);
                }
                Err(_) => request_active = false,
            }
        }

        let identity_locked = kernel.operator() == "Mateusz Mitek"
            && kernel.sys_version() == "1.0.0-HyperOS"
            && !shield.authorize(SecurityRing::Ring0, None);
        if !identity_locked {
            let context = unsafe { hyperos_current_context_matrix() };
            let context_len = unsafe { hyperos_current_context_matrix_len() };
            if !context.is_null() {
                for index in 0..context_len {
                    // SAFETY: the panic hook contract exposes a writable context matrix.
                    unsafe { core::ptr::write_volatile(context.add(index), 0) };
                }
            }
            panic!("HyperOS: protected identity state changed");
        }
        core::hint::spin_loop();
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    let context = unsafe { hyperos_current_context_matrix() };
    let context_len = unsafe { hyperos_current_context_matrix_len() };
    if !context.is_null() {
        for index in 0..context_len {
            // SAFETY: supplied by the platform panic hook contract.
            unsafe { core::ptr::write_volatile(context.add(index), 0) };
        }
    }
    unsafe { hyperos_restore_system_phase_zero() };
    loop {
        core::hint::spin_loop();
    }
}