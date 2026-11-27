//! Copies from title memory into a host buffer. Reading through the kernel
//! (`ReadProcessMemory`) survives concurrent unmaps, but pays a syscall and page
//! table walk on every read. On x86_64 we can recognize a missing page at one
//! copy instruction and fall back to the kernel only on fault. Set
//! `VICTORY_COPY_PLAIN=0` to force the kernel path.

use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Path {
    Plain,
    Fault,
    Small,
    Cooling,
    Disabled,
}

static COUNTS: [AtomicU64; 5] = [const { AtomicU64::new(0) }; 5];

/// Counts per requested host read since the last query, divided by `frames`.
#[allow(dead_code)]
pub(crate) fn per_frame(frames: f64) -> [f64; 5] {
    COUNTS
        .each_ref()
        .map(|n| n.swap(0, Relaxed) as f64 / frames)
}

/// Copies `len` bytes from `src` into host buffer `dst`, returning `true` when
/// all bytes succeeded in place. On `false`, the caller retries through the
/// kernel (including any prefix already written).
///
/// # Safety
/// `dst` must be a writable host allocation for `len` bytes, disjoint from
/// title memory. `src` may be unmapped or changed concurrently by the title.
pub(crate) unsafe fn copy(src: *const u8, dst: *mut u8, len: usize) -> bool {
    #[cfg(target_arch = "x86_64")]
    let path = {
        static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if *ON.get_or_init(|| {
            std::env::var_os("VICTORY_COPY_PLAIN").is_none_or(|v| v != "0") && x64::install()
        }) {
            // SAFETY: caller supplies the writable host buffer and fault handler is installed.
            unsafe { x64::attempt(src, dst, len, &x64::FAULTS) }
        } else {
            Path::Disabled
        }
    };
    #[cfg(not(target_arch = "x86_64"))]
    let path = Path::Disabled;
    COUNTS[path as usize].fetch_add(1, Relaxed);
    path == Path::Plain
}

#[cfg(target_arch = "x86_64")]
mod x64 {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::{OnceLock, RwLock};
    use std::time::Instant;
    use windows_sys::Win32::Foundation::{
        EXCEPTION_ACCESS_VIOLATION, EXCEPTION_GUARD_PAGE, EXCEPTION_IN_PAGE_ERROR,
    };
    use windows_sys::Win32::System::Diagnostics::Debug::{
        AddVectoredExceptionHandler, CONTEXT, EXCEPTION_CONTINUE_EXECUTION,
        EXCEPTION_CONTINUE_SEARCH, EXCEPTION_POINTERS, EXCEPTION_RECORD,
    };

    core::arch::global_asm!(
        ".text",
        ".globl victory_plain_copy",
        ".globl victory_plain_copy_at",
        ".globl victory_plain_copy_exit",
        ".seh_proc victory_plain_copy",
        "victory_plain_copy:",
        "push rsi",
        ".seh_pushreg rsi",
        "push rdi",
        ".seh_pushreg rdi",
        "sub rsp, 8",
        ".seh_stackalloc 8",
        ".seh_endprologue",
        ".seh_handler {unclaimed_guard}, @except",
        "mov rdi, rcx",
        "mov rsi, rdx",
        "mov rcx, r8",
        "victory_plain_copy_at:",
        "rep movsb",
        "xor eax, eax",
        "victory_plain_copy_exit:",
        "add rsp, 8",
        "pop rdi",
        "pop rsi",
        "ret",
        ".seh_endproc",
        unclaimed_guard = sym unclaimed_guard,
    );

    unsafe extern "C" {
        fn victory_plain_copy(dst: *mut u8, src: *const u8, len: usize) -> usize;
        static victory_plain_copy_at: u8;
        static victory_plain_copy_exit: u8;
    }

    /// View guards in `watch` use `PAGE_READONLY`. Reading a guarded source
    /// leaves it armed; write violations must continue to the guard handler.
    fn recover_read(record: &EXCEPTION_RECORD, context: &mut CONTEXT) -> bool {
        if context.Rip != (&raw const victory_plain_copy_at) as u64
            || record.NumberParameters < 2
            || record.ExceptionInformation[0] != 0
        {
            return false;
        }
        context.Rax = (record.ExceptionInformation[1] as u64 >> 16) + 1;
        context.Rip = (&raw const victory_plain_copy_exit) as u64;
        true
    }

    unsafe extern "system" fn on_fault(info: *mut EXCEPTION_POINTERS) -> i32 {
        // SAFETY: Windows supplies valid exception record and context pointers.
        let (record, context) = unsafe { (&*(*info).ExceptionRecord, &mut *(*info).ContextRecord) };
        if matches!(
            record.ExceptionCode,
            EXCEPTION_ACCESS_VIOLATION | EXCEPTION_IN_PAGE_ERROR
        ) && recover_read(record, context)
        {
            EXCEPTION_CONTINUE_EXECUTION
        } else {
            EXCEPTION_CONTINUE_SEARCH
        }
    }

    unsafe extern "system" fn unclaimed_guard(
        record: *const EXCEPTION_RECORD,
        _frame: *mut std::ffi::c_void,
        context: *mut CONTEXT,
        _dispatcher: *mut std::ffi::c_void,
    ) -> i32 {
        use windows_sys::Win32::System::Kernel::{
            ExceptionContinueExecution, ExceptionContinueSearch,
        };
        // SAFETY: Windows calls this handler for the SEH frame above.
        let (record, context) = unsafe { (&*record, &mut *context) };
        if record.ExceptionCode == EXCEPTION_GUARD_PAGE && recover_read(record, context) {
            ExceptionContinueExecution
        } else {
            ExceptionContinueSearch
        }
    }

    pub(super) fn install() -> bool {
        static INSTALLED: OnceLock<bool> = OnceLock::new();
        *INSTALLED.get_or_init(|| {
            // SAFETY: function remains loaded for process lifetime.
            let installed = !unsafe { AddVectoredExceptionHandler(1, Some(on_fault)) }.is_null();
            if !installed {
                tracing::warn!("could not install plain copy handler, using ReadProcessMemory");
            }
            installed
        })
    }

    fn now_ms() -> u64 {
        static START: OnceLock<Instant> = OnceLock::new();
        START.get_or_init(Instant::now).elapsed().as_millis() as u64
    }

    /// Per-64 KiB block fault cooldown (2 s) so repeated unmapped reads go
    /// directly to the kernel path without incurring fault overhead.
    pub(super) struct Faults {
        latest: AtomicU64,
        recent: RwLock<Recent>,
    }

    struct Recent {
        deadlines: BTreeMap<usize, u64>,
        prune_at: u64,
    }

    pub(super) static FAULTS: Faults = Faults::new();

    impl Faults {
        const fn new() -> Self {
            Self {
                latest: AtomicU64::new(0),
                recent: RwLock::new(Recent {
                    deadlines: BTreeMap::new(),
                    prune_at: 0,
                }),
            }
        }

        fn cooling(&self, start: usize, len: usize, now: u64) -> bool {
            if self.latest.load(Relaxed) <= now {
                return false;
            }
            let blocks = (start >> 16)..=(start.saturating_add(len - 1) >> 16);
            self.recent
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .deadlines
                .range(blocks)
                .any(|(_, until)| *until > now)
        }

        fn remember(&self, block: usize, now: u64) {
            let expires = now + 2_000;
            let mut recent = self.recent.write().unwrap_or_else(|e| e.into_inner());
            if now >= recent.prune_at {
                recent.deadlines.retain(|_, until| *until > now);
                recent.prune_at = expires;
            }
            let until = recent.deadlines.entry(block).or_default();
            *until = (*until).max(expires);
            self.latest.fetch_max(*until, Relaxed);
        }
    }

    /// Reads under 256 bytes stay on the kernel path because tiny residency
    /// probes intentionally touch unmapped pages.
    pub(super) unsafe fn attempt(
        src: *const u8,
        dst: *mut u8,
        len: usize,
        faults: &Faults,
    ) -> Path {
        if len < 256 {
            return Path::Small;
        }
        if faults.cooling(src as usize, len, now_ms()) {
            return Path::Cooling;
        }
        // SAFETY: caller owns `dst` and vectored handler is installed.
        match unsafe { victory_plain_copy(dst, src, len) } {
            0 => Path::Plain,
            fault => {
                faults.remember(fault - 1, now_ms());
                Path::Fault
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use windows_sys::Win32::System::Diagnostics::Debug::ReadProcessMemory;
        use windows_sys::Win32::System::Memory::{
            MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_GUARD, PAGE_NOACCESS, PAGE_READWRITE,
            VirtualAlloc, VirtualFree, VirtualProtect,
        };
        use windows_sys::Win32::System::Threading::GetCurrentProcess;

        const PAGE: usize = 4096;

        #[test]
        fn only_reads_of_at_least_256_bytes_try_the_instruction() {
            assert!(install());
            let src = [0xb5; 257];
            let faults = Faults::new();
            for n in [0, 1, 255, 256, 257] {
                let mut dst = [0; 257];
                // SAFETY: separate arrays, large enough for every read.
                let path = unsafe { attempt(src.as_ptr(), dst.as_mut_ptr(), n, &faults) };
                if n < 256 {
                    assert_eq!(path, Path::Small);
                    assert_eq!(dst, [0; 257]);
                } else {
                    assert_eq!(path, Path::Plain);
                    assert_eq!(dst[..n], src[..n]);
                    assert!(dst[n..].iter().all(|&v| v == 0));
                }
            }
        }

        #[test]
        fn each_fault_cools_its_own_block_for_two_seconds() {
            let faults = Faults::new();
            faults.remember(2, 1_999);
            faults.remember(4, 3_000);
            assert!(faults.cooling(1 << 16, 2 << 16, 3_998));
            assert!(!faults.cooling(1 << 16, 1 << 16, 3_998));
            assert!(!faults.cooling(2 << 16, 1, 3_999));
            assert!(faults.cooling(4 << 16, 1, 4_999));
            assert!(!faults.cooling(4 << 16, 1, 5_000));
        }

        #[test]
        fn a_missing_middle_page_retries_the_kernel_and_preserves_both_islands() {
            const SIZE: usize = 3 * 65536;
            const HOLE: usize = 65536 + PAGE;
            assert!(install());
            let faults = Faults::new();
            let mut dst = vec![0xcc; SIZE];
            let mut kernel = vec![0xcc; SIZE];
            // SAFETY: reserve three blocks and commit all but one interior page.
            unsafe {
                let src =
                    VirtualAlloc(std::ptr::null(), SIZE, MEM_RESERVE, PAGE_NOACCESS).cast::<u8>();
                assert!(!src.is_null());
                for (at, n, byte) in [(0, HOLE, 0x47), (HOLE + PAGE, SIZE - HOLE - PAGE, 0xad)] {
                    assert_eq!(
                        VirtualAlloc(src.add(at).cast(), n, MEM_COMMIT, PAGE_READWRITE),
                        src.add(at).cast()
                    );
                    src.add(at).write_bytes(byte, n);
                }
                assert_eq!(attempt(src, dst.as_mut_ptr(), SIZE, &faults), Path::Fault);
                assert_eq!(attempt(src, dst.as_mut_ptr(), SIZE, &faults), Path::Cooling);
                assert_eq!(attempt(src, dst.as_mut_ptr(), PAGE, &faults), Path::Plain);
                let rpm = |at: usize, to: &mut [u8]| {
                    let mut bytes = 0;
                    let whole = ReadProcessMemory(
                        GetCurrentProcess(),
                        src.add(at).cast(),
                        to.as_mut_ptr().cast(),
                        to.len(),
                        &mut bytes,
                    ) != 0;
                    (whole, bytes)
                };
                let (whole, bytes) = rpm(0, &mut kernel);
                assert!(!whole);
                dst.fill(0xcc);
                assert_eq!(crate::host::read_into(src, &mut dst), whole);
                assert_eq!(dst[..bytes], kernel[..bytes]);
                for (i, page) in kernel.chunks_exact_mut(PAGE).enumerate() {
                    if !rpm(i * PAGE, page).0 {
                        page.fill(0);
                    }
                }
                dst.fill(0xcc);
                crate::host::copy_mapped(dst.as_mut_ptr(), src, SIZE, false);
                assert_eq!(dst, kernel);
                assert_ne!(VirtualFree(src.cast(), 0, MEM_RELEASE), 0);
            }
        }

        #[test]
        fn reading_a_guarded_view_leaves_the_next_write_for_the_guard_handler() {
            use crate::watch::tests::{drop_views, guard, two_views};
            const BLOCK: usize = 1 << 16;
            let (section, view, alias) = two_views(BLOCK);
            let changes = || guard(view, view + BLOCK).unwrap();
            assert_eq!(changes(), [view / crate::watch::PAGE]);
            assert!(install());
            let faults = Faults::new();
            let mut dst = [0x7b; 256];
            // SAFETY: view is readable; second copy intentionally writes its alias at REP MOVSB.
            unsafe {
                assert_eq!(
                    attempt(view as *const u8, dst.as_mut_ptr(), dst.len(), &faults),
                    Path::Plain
                );
                assert_eq!(dst, [0; 256]);
                assert!(changes().is_empty());
                dst.fill(0x7b);
                assert_eq!(
                    attempt(dst.as_ptr(), alias as *mut u8, dst.len(), &faults),
                    Path::Plain
                );
                assert_eq!(changes(), [view / crate::watch::PAGE]);
                assert_eq!(
                    std::slice::from_raw_parts(view as *const u8, dst.len()),
                    dst
                );
            }
            drop_views(section, &[view, alias], BLOCK);
        }

        #[test]
        fn an_unclaimed_windows_guard_falls_back_at_the_copy() {
            assert!(install());
            let mut dst = [0; PAGE];
            // SAFETY: fresh allocation with one-shot PAGE_GUARD.
            unsafe {
                let src = VirtualAlloc(
                    std::ptr::null(),
                    PAGE,
                    MEM_RESERVE | MEM_COMMIT,
                    PAGE_READWRITE,
                );
                assert!(!src.is_null());
                let mut old = 0;
                assert_ne!(
                    VirtualProtect(src, PAGE, PAGE_READWRITE | PAGE_GUARD, &mut old),
                    0
                );
                assert_eq!(
                    attempt(src.cast(), dst.as_mut_ptr(), PAGE, &Faults::new()),
                    Path::Fault
                );
                assert_ne!(VirtualFree(src, 0, MEM_RELEASE), 0);
            }
        }
    }
}
