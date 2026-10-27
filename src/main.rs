#![no_std]
#![no_main]

use core::mem::MaybeUninit;
use core::panic::PanicInfo;
use core::ptr::NonNull;
use sha2::{Digest, Sha256};

use hyperos_core::chrono_log::{ChronoLogEngine, LogEntry};
use hyperos_core::hyper_allocator::HyperAllocator;
use hyperos_core::hyper_bridge_loop::{HyperBridgeLoop, LoopProgress, NonBlockingOutput};
use hyperos_core::hyper_chrono_guard::HyperChronoGuard;
use hyperos_core::hyper_diagnostics::run_post_validation;
use hyperos_core::hyper_euler_engine::compute_euler_rotation;
use hyperos_core::hyper_ollama_client::{IoError, NonBlockingSocket};
use hyperos_core::hyper_scheduler::HyperScheduler;
use hyperos_core::hyper_shield::{HyperShieldController, SecurityRing};
use hyperos_core::hyper_update_engine::{AuthenticatedUpdateSource, HyperUpdateEngine, UpdateProgress};
use hyperos_core::hyper_vision::HyperVisionEngine;
use hyperos_core::hyper_model::{GgmlType, GgufTensor, TensorInfo};
use hyperos_core::sanitized_stream_reader::SanitizedStreamReader;
use hyperos_core::terminal::{HyperSpaceTerminal, VECTOR_BYTES};
use hyperos_core::kernel::HyperKernel;

const BLOCK_SIZE: usize = 8 * 1024 * 1024;
const POOL_SEGMENTS: usize = 60;
const ALLOCATOR_BLOCKS: usize = 64;
const SEGMENT_BYTES: usize = BLOCK_SIZE * ALLOCATOR_BLOCKS;
const REQUIRED_POOL_BYTES: u64 = 30 * 1024 * 1024 * 1024;
const REQUIRED_KV_BYTES: u64 = 16 * 1024 * 1024 * 1024;
const RESPONSE_CAPACITY: usize = 4096;

static mut RESPONSE_BUFFER: [u8; RESPONSE_CAPACITY] = [0; RESPONSE_CAPACITY];
static mut PREBOOT_POOL_BASE: *const u8 = core::ptr::null();
static mut PREBOOT_POOL_LENGTH: usize = 0;
static mut PREBOOT_OPERATOR_BASE: *const u8 = core::ptr::null();
static mut PREBOOT_OPERATOR_LENGTH: usize = 0;

/// All functions below are supplied by the target's bootloader/platform layer.
/// Input slices remain valid and immutable until the next input poll.
extern "C" {
    fn hyperos_platform_memory_pool(base: *mut *mut u8, length: *mut usize) -> i32;
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

fn idle_task() {}

fn log_event(log: &ChronoLogEngine, entry: LogEntry) {
    if log.push(entry).is_err() {
        let _ = log.pop();
        let _ = log.push(entry);
    }
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

#[no_mangle]
pub extern "C" fn _start() -> ! {
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
    let kv_entry_count = kv_length / VECTOR_BYTES;
    let kv_cache: &'static mut [[u8; VECTOR_BYTES]] = unsafe {
        core::slice::from_raw_parts_mut(kv_base.cast(), kv_entry_count)
    };

    let terminal = HyperSpaceTerminal::new();
    let mut scheduler = HyperScheduler::new();
    if scheduler.register(0, idle_task).is_err() {
        panic!("HyperOS: scheduler initialization failed");
    }
    let mut shield = HyperShieldController::new();
    let mut update_engine = HyperUpdateEngine::new();
    let mut update_guard = HyperChronoGuard::new();
    let mut chat_log = ChronoLogEngine::new();
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

    loop {
        let ticks = unsafe { hyperos_clock_ticks() };

        let phase = (ticks & 7) as f64;
        let _ = scheduler.run_slice(phase);

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