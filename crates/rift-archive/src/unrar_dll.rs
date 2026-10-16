//! The slice of UnRAR's DLL API rift-archive needs, made safe.
//!
//! `unrar_sys` compiles the official UnRAR C++ sources (UnRAR 7.x, DLL API
//! version 9) and declares their functions. This module calls those symbols
//! through its own struct mirrors instead of the `unrar` crate's safe API,
//! for two reasons found in the vendored sources:
//!
//! 1. **Cancelling a read.** The `unrar` crate's data callback always answers
//!    "continue", so `read()` inflates a whole entry into memory. UnRAR stops
//!    decompressing only when the `UCM_PROCESSDATA` callback returns -1
//!    (rdwrfn.cpp `ComprDataIO::UnpWrite` → `ErrHandler.Exit(RARX_USERBREAK)`,
//!    caught by `ProcessFile`, which then answers `ERAR_UNKNOWN`). That is how
//!    `read_entry`'s `limit` stops a 2 GB pak after its table of contents.
//! 2. **Struct layout.** dll.hpp declares every struct under
//!    `#pragma pack(push, 1)`; unrar_sys 0.5.8 mirrors them as plain
//!    `#[repr(C)]`. On 64-bit targets the mirrors drift by 4 bytes at the
//!    first pointer field: `CmtBuf` in `RARHeaderDataEx`, `CmtBufW` in
//!    `RAROpenArchiveDataEx`. The `unrar` crate never reads past the drift,
//!    so it works; this module needs `RedirType` (UnRAR streams no bytes for
//!    a link entry under `RAR_TEST`, extract.cpp: "Assume success for test
//!    mode") and the `FileNameEx` / `RedirName` buffers, so it carries packed
//!    mirrors, checked against dll.hpp's sizes at compile time.
//!
//! Callbacks: every DLL call that may call back gets a fresh context through
//! `RARSetCallback` and the callback is detached again before the context
//! leaves scope, so UnRAR never holds a dangling pointer. The callback never
//! unwinds into C++: a panic is caught and turned into "cancel".
//!
//! Threads: one handle is used by one thread at a time (`&mut self`). UnRAR
//! keeps a process-wide error-code slot that concurrent handles race on; it
//! only colours the error code of a failed open, never the bytes.

use std::mem::size_of;
use std::os::raw::{c_char, c_int, c_uint};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::ptr::{self, NonNull};

use unrar_sys as sys;

type WChar = sys::WCHAR;

/// UnRAR's MAXPATHSIZE (rardefs.hpp): no name in an archive is longer, so a
/// buffer of this size never truncates one. (The DLL ignores a `RedirName`
/// buffer of 100 000 characters or more; this stays below.)
const NAME_CHARS: usize = 0x10000 + 1;

/// `RARHeaderDataEx::UnpSize` when the archive does not know the size
/// (INT64NDF in UnRAR: a file archived from a pipe).
const UNKNOWN_SIZE: u64 = (0x7fff_ffff_u64 << 32) | 0x7fff_ffff;

/// UnRAR's `FILE_SYSTEM_REDIRECT` (headers.hpp).
const FSREDIR_NONE: c_uint = 0;
const FSREDIR_UNIXSYMLINK: c_uint = 1;
const FSREDIR_WINSYMLINK: c_uint = 2;
const FSREDIR_JUNCTION: c_uint = 3;
const FSREDIR_HARDLINK: c_uint = 4;
const FSREDIR_FILECOPY: c_uint = 5;

/// dll.hpp `struct RAROpenArchiveDataEx`, byte-packed like the original.
#[repr(C, packed)]
#[allow(dead_code)] // a layout mirror: UnRAR reads and writes these fields
struct OpenArchiveDataEx {
    arc_name: *const c_char,
    arc_name_w: *const WChar,
    open_mode: c_uint,
    open_result: c_uint,
    cmt_buf: *mut c_char,
    cmt_buf_size: c_uint,
    cmt_size: c_uint,
    cmt_state: c_uint,
    flags: c_uint,
    callback: Option<sys::Callback>,
    user_data: sys::LPARAM,
    op_flags: c_uint,
    cmt_buf_w: *mut WChar,
    reserved: [c_uint; 25],
}

/// dll.hpp `struct RARHeaderDataEx`, byte-packed like the original.
#[repr(C, packed)]
#[allow(dead_code)] // a layout mirror: UnRAR reads and writes these fields
struct HeaderDataEx {
    arc_name: [c_char; 1024],
    arc_name_w: [WChar; 1024],
    file_name: [c_char; 1024],
    file_name_w: [WChar; 1024],
    flags: c_uint,
    pack_size: c_uint,
    pack_size_high: c_uint,
    unp_size: c_uint,
    unp_size_high: c_uint,
    host_os: c_uint,
    file_crc: c_uint,
    file_time: c_uint,
    unp_ver: c_uint,
    method: c_uint,
    file_attr: c_uint,
    cmt_buf: *mut c_char,
    cmt_buf_size: c_uint,
    cmt_size: c_uint,
    cmt_state: c_uint,
    dict_size: c_uint,
    hash_type: c_uint,
    hash: [c_char; 32],
    redir_type: c_uint,
    redir_name: *mut WChar,
    redir_name_size: c_uint,
    dir_target: c_uint,
    mtime_low: c_uint,
    mtime_high: c_uint,
    ctime_low: c_uint,
    ctime_high: c_uint,
    atime_low: c_uint,
    atime_high: c_uint,
    arc_name_ex: *mut WChar,
    arc_name_ex_size: c_uint,
    file_name_ex: *mut WChar,
    file_name_ex_size: c_uint,
    reserved: [c_uint; 982],
}

// The packed mirrors must be exactly dll.hpp's sizes: 4-byte ints, pointers,
// wchar_t (2 bytes on Windows, 4 elsewhere), LPARAM.
const POINTER: usize = size_of::<*const u8>();
const _: () = assert!(
    size_of::<OpenArchiveDataEx>()
        == 5 * POINTER // ArcName, ArcNameW, CmtBuf, Callback, CmtBufW
            + size_of::<sys::LPARAM>() // UserData
            + 7 * 4 // OpenMode, OpenResult, CmtBufSize, CmtSize, CmtState, Flags, OpFlags
            + 25 * 4 // Reserved
);
const _: () = assert!(
    size_of::<HeaderDataEx>()
        == 2 * 1024 // ArcName, FileName
            + 2 * 1024 * size_of::<WChar>() // ArcNameW, FileNameW
            + 4 * POINTER // CmtBuf, RedirName, ArcNameEx, FileNameEx
            + 27 * 4 // the unsigned ints from Flags to FileNameExSize
            + 32 // Hash
            + 982 * 4 // Reserved
);

/// Where streamed bytes go: answers whether UnRAR should go on.
pub(crate) type DataSink<'s> = dyn FnMut(&[u8]) -> bool + 's;

/// How an archive is opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Headers only; skipping an entry is a seek, even in a solid archive.
    List,
    /// Headers and data; in a solid archive skipping an entry decompresses it.
    Extract,
}

/// What a failed DLL call left behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Failure {
    /// The `ERAR_*` code.
    pub code: c_int,
    /// UnRAR asked for the next volume of a multi-part archive and it is not
    /// there.
    pub volume_missing: bool,
    /// UnRAR asked for a password.
    pub password_asked: bool,
    /// The data sink panicked (the read was cancelled).
    pub panicked: bool,
}

impl Failure {
    fn code(code: c_int) -> Self {
        Failure {
            code,
            volume_missing: false,
            password_asked: false,
            panicked: false,
        }
    }

    fn with(code: c_int, signals: Signals) -> Self {
        Failure {
            code,
            volume_missing: signals.volume_missing,
            password_asked: signals.password_asked,
            panicked: signals.panicked,
        }
    }
}

/// A file header, decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Header {
    /// The name exactly as UnRAR reports it (native separators).
    pub name: String,
    /// Uncompressed size; `None` when the archive does not know it.
    pub size: Option<u64>,
    pub directory: bool,
    pub encrypted: bool,
    /// A continuation of a file begun in the previous volume.
    pub split_before: bool,
    pub link: Link,
}

/// What kind of file-system object a header describes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Link {
    /// A plain file (or folder).
    None,
    /// A symbolic link or junction: never a file to install.
    Symlink,
    /// A hard link or a stored copy of an earlier entry, named here; its bytes
    /// are that entry's bytes.
    CopyOf(String),
    /// A redirect kind this module does not know.
    Unknown(c_uint),
}

/// How a streamed entry ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Streamed {
    /// UnRAR delivered the whole entry and its checksum matched.
    Complete,
    /// The sink asked to stop; the handle is spent.
    Stopped,
}

/// What the callback saw during one DLL call.
#[derive(Debug, Default, Clone, Copy)]
struct Signals {
    sink_stopped: bool,
    volume_missing: bool,
    password_asked: bool,
    panicked: bool,
}

/// The callback's view of the call in progress.
struct CallbackContext<'r, 's> {
    sink: Option<&'r mut DataSink<'s>>,
    signals: Signals,
}

/// The single callback UnRAR is ever given. `user_data` is the address of a
/// live [`CallbackContext`] (see [`with_callback`]).
extern "C" fn callback(
    msg: sys::UINT,
    user_data: sys::LPARAM,
    p1: sys::LPARAM,
    p2: sys::LPARAM,
) -> c_int {
    if user_data == 0 {
        // No context: refuse anything that asks for an answer.
        return -1;
    }
    // SAFETY: `user_data` was set by `with_callback` (or `open`) to the
    // address of a `CallbackContext` that outlives the DLL call in progress;
    // the callback is detached before that context is dropped, and UnRAR
    // calls back only on the calling thread.
    let context = unsafe { &mut *(user_data as *mut CallbackContext<'_, '_>) };
    match catch_unwind(AssertUnwindSafe(|| dispatch(context, msg, p1, p2))) {
        Ok(answer) => answer,
        Err(_) => {
            context.signals.panicked = true;
            -1
        }
    }
}

fn dispatch(
    context: &mut CallbackContext<'_, '_>,
    msg: sys::UINT,
    p1: sys::LPARAM,
    p2: sys::LPARAM,
) -> c_int {
    match msg {
        sys::UCM_PROCESSDATA => {
            let Some(sink) = context.sink.as_mut() else {
                return 1;
            };
            let len = usize::try_from(p2).unwrap_or(0);
            if p1 == 0 || len == 0 {
                return 1;
            }
            // SAFETY: for UCM_PROCESSDATA, P1 is the address of UnRAR's
            // unpacked-data buffer and P2 its length, valid (and not modified)
            // for the duration of this call (RARCallback.md).
            let data = unsafe { std::slice::from_raw_parts(p1 as *const u8, len) };
            if sink(data) {
                1
            } else {
                context.signals.sink_stopped = true;
                -1
            }
        }
        sys::UCM_CHANGEVOLUME | sys::UCM_CHANGEVOLUMEW => {
            if p2 == sys::RAR_VOL_ASK {
                // The next volume is missing: answering "retry" would loop.
                context.signals.volume_missing = true;
                -1
            } else {
                1
            }
        }
        sys::UCM_NEEDPASSWORD | sys::UCM_NEEDPASSWORDW => {
            context.signals.password_asked = true;
            -1
        }
        _ => 0,
    }
}

/// Run one DLL call with a fresh callback context, detaching it afterwards.
fn with_callback<R>(
    handle: NonNull<sys::Handle>,
    sink: Option<&mut DataSink<'_>>,
    call: impl FnOnce(*const sys::Handle) -> R,
) -> (R, Signals) {
    let mut context = CallbackContext {
        sink,
        signals: Signals::default(),
    };
    let raw = handle.as_ptr().cast_const();
    // SAFETY: `raw` is an open handle; the context address stays valid until
    // the second RARSetCallback below detaches it.
    unsafe {
        sys::RARSetCallback(
            raw,
            Some(callback),
            ptr::addr_of_mut!(context) as sys::LPARAM,
        );
    }
    let result = call(raw);
    // SAFETY: as above; detaching leaves UnRAR without a pointer to `context`.
    unsafe { sys::RARSetCallback(raw, None, 0) };
    (result, context.signals)
}

/// An open RAR archive.
pub(crate) struct RarArchive {
    handle: NonNull<sys::Handle>,
    flags: c_uint,
    header: Box<HeaderDataEx>,
    name: Vec<WChar>,
    redirect: Vec<WChar>,
    /// Set once a read was cancelled mid-entry or a call failed: the DLL's
    /// position is no longer at a header, so nothing more is asked of it.
    spent: bool,
}

impl RarArchive {
    /// Open `path` in `mode`.
    pub(crate) fn open(path: &Path, mode: Mode) -> Result<Self, Failure> {
        let name = ArchiveName::new(path).ok_or(Failure::code(sys::ERAR_EOPEN))?;
        let mut context = CallbackContext {
            sink: None,
            signals: Signals::default(),
        };
        // SAFETY: every field of the mirror is an integer, a raw pointer or an
        // `Option<fn>`; all-zero is a valid value for each (dll.hpp asks for
        // the reserved area to be zeroed).
        let mut data: OpenArchiveDataEx = unsafe { std::mem::zeroed() };
        name.install(&mut data);
        data.open_mode = match mode {
            Mode::List => sys::RAR_OM_LIST,
            Mode::Extract => sys::RAR_OM_EXTRACT,
        };
        data.callback = Some(callback);
        data.user_data = ptr::addr_of_mut!(context) as sys::LPARAM;
        // SAFETY: `data` is a correctly laid-out RAROpenArchiveDataEx whose
        // name pointer (`name`) and callback context (`context`) outlive the
        // call; UnRAR copies the name and keeps only the callback, which is
        // detached right after.
        let raw = unsafe {
            sys::RAROpenArchiveEx(ptr::addr_of_mut!(data).cast::<sys::OpenArchiveDataEx>())
        };
        let handle = NonNull::new(raw.cast_mut());
        if let Some(handle) = handle {
            // SAFETY: `handle` was just returned by RAROpenArchiveEx.
            unsafe { sys::RARSetCallback(handle.as_ptr(), None, 0) };
        }
        drop(name);
        let code = data.open_result as c_int;
        let flags = data.flags;
        match (handle, code) {
            (Some(handle), sys::ERAR_SUCCESS) => Ok(RarArchive {
                handle,
                flags,
                header: zeroed_header(),
                name: vec![0; NAME_CHARS],
                redirect: vec![0; NAME_CHARS],
                spent: false,
            }),
            (Some(handle), _) => {
                // A handle can come back with an error (not an archive after
                // all); it still has to be closed.
                // SAFETY: `handle` is open and closed exactly once.
                unsafe { sys::RARCloseArchive(handle.as_ptr()) };
                Err(Failure::with(code, context.signals))
            }
            (None, sys::ERAR_SUCCESS) => Err(Failure::code(sys::ERAR_UNKNOWN)),
            (None, _) => Err(Failure::with(code, context.signals)),
        }
    }

    /// Solid: entries share one compressed stream; reaching an entry means
    /// decompressing everything before it.
    pub(crate) fn is_solid(&self) -> bool {
        self.flags & sys::ROADF_SOLID != 0
    }

    /// Read the next file header; `None` at the end of the archive.
    pub(crate) fn read_header(&mut self) -> Result<Option<Header>, Failure> {
        self.check_usable()?;
        // SAFETY: all-zero is valid for the mirror (see `zeroed_header`); the
        // DLL requires the reserved area zeroed on every call.
        unsafe { ptr::write_bytes(ptr::addr_of_mut!(*self.header), 0, 1) };
        self.name[0] = 0;
        self.redirect[0] = 0;
        self.header.file_name_ex = self.name.as_mut_ptr();
        self.header.file_name_ex_size = NAME_CHARS as c_uint;
        self.header.redir_name = self.redirect.as_mut_ptr();
        self.header.redir_name_size = NAME_CHARS as c_uint;
        let header = ptr::addr_of_mut!(*self.header).cast::<sys::HeaderDataEx>();
        let (code, signals) = with_callback(self.handle, None, |raw| {
            // SAFETY: `raw` is open; `header` points at a correctly laid-out,
            // zeroed RARHeaderDataEx whose name buffers (`self.name`,
            // `self.redirect`) are as long as the sizes it declares.
            unsafe { sys::RARReadHeaderEx(raw, header.cast_const()) }
        });
        match code {
            sys::ERAR_SUCCESS => Ok(Some(self.decode_header())),
            sys::ERAR_END_ARCHIVE => Ok(None),
            _ => {
                self.spent = true;
                Err(Failure::with(code, signals))
            }
        }
    }

    /// Move past the entry whose header was just read without delivering it.
    pub(crate) fn skip(&mut self) -> Result<(), Failure> {
        self.check_usable()?;
        let (code, signals) = with_callback(self.handle, None, |raw| {
            // SAFETY: `raw` is open and positioned after a header.
            unsafe { sys::RARProcessFileW(raw, sys::RAR_SKIP, ptr::null(), ptr::null()) }
        });
        if code == sys::ERAR_SUCCESS {
            Ok(())
        } else {
            self.spent = true;
            Err(Failure::with(code, signals))
        }
    }

    /// Deliver the entry whose header was just read to `sink`, chunk by
    /// chunk, verifying its checksum at the end (`RAR_TEST`: nothing touches
    /// the disk). `sink` answers whether to go on; answering `false` stops
    /// decompression there and spends the handle.
    pub(crate) fn stream(&mut self, sink: &mut DataSink<'_>) -> Result<Streamed, Failure> {
        self.check_usable()?;
        let (code, signals) = with_callback(self.handle, Some(sink), |raw| {
            // SAFETY: `raw` is open and positioned after a header.
            unsafe { sys::RARProcessFileW(raw, sys::RAR_TEST, ptr::null(), ptr::null()) }
        });
        if signals.panicked {
            self.spent = true;
            return Err(Failure::with(sys::ERAR_UNKNOWN, signals));
        }
        if signals.sink_stopped {
            self.spent = true;
            return Ok(Streamed::Stopped);
        }
        if code == sys::ERAR_SUCCESS {
            Ok(Streamed::Complete)
        } else {
            self.spent = true;
            Err(Failure::with(code, signals))
        }
    }

    fn check_usable(&self) -> Result<(), Failure> {
        if self.spent {
            Err(Failure::code(sys::ERAR_UNKNOWN))
        } else {
            Ok(())
        }
    }

    fn decode_header(&self) -> Header {
        let flags = self.header.flags;
        let raw_size =
            (u64::from(self.header.unp_size_high) << 32) | u64::from(self.header.unp_size);
        let mut name = wide_to_string(&self.name);
        if name.is_empty() {
            // A DLL without FileNameEx support leaves our buffer untouched;
            // fall back to the fixed 1024-character field.
            let fixed = self.header.file_name_w;
            name = wide_to_string(&fixed);
        }
        let link = match self.header.redir_type {
            FSREDIR_NONE => Link::None,
            FSREDIR_UNIXSYMLINK | FSREDIR_WINSYMLINK | FSREDIR_JUNCTION => Link::Symlink,
            FSREDIR_HARDLINK | FSREDIR_FILECOPY => Link::CopyOf(wide_to_string(&self.redirect)),
            other => Link::Unknown(other),
        };
        Header {
            name,
            size: (raw_size != UNKNOWN_SIZE).then_some(raw_size),
            directory: flags & sys::RHDF_DIRECTORY != 0,
            encrypted: flags & sys::RHDF_ENCRYPTED != 0,
            split_before: flags & sys::RHDF_SPLITBEFORE != 0,
            link,
        }
    }
}

impl Drop for RarArchive {
    fn drop(&mut self) {
        // SAFETY: the handle is open (RarArchive exists only for open
        // handles) and is closed exactly once, here.
        unsafe {
            sys::RARSetCallback(self.handle.as_ptr(), None, 0);
            sys::RARCloseArchive(self.handle.as_ptr());
        }
    }
}

fn zeroed_header() -> Box<HeaderDataEx> {
    // SAFETY: every field of the mirror is an integer, an integer array or a
    // raw pointer; all-zero is a valid value for each. (Built on the stack and
    // moved: `Box::new_zeroed` is newer than the workspace's MSRV.)
    Box::new(unsafe { std::mem::zeroed::<HeaderDataEx>() })
}

/// The archive path in the form UnRAR wants it: UTF-16 on Windows; on other
/// systems the raw bytes, which UnRAR maps losslessly even in the "C" locale
/// (the same choice the `unrar` crate makes on Linux).
struct ArchiveName {
    #[cfg(windows)]
    wide: Vec<u16>,
    #[cfg(not(windows))]
    narrow: std::ffi::CString,
}

impl ArchiveName {
    fn new(path: &Path) -> Option<Self> {
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
            if wide.contains(&0) {
                return None;
            }
            wide.push(0);
            Some(ArchiveName { wide })
        }
        #[cfg(not(windows))]
        {
            let narrow = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).ok()?;
            Some(ArchiveName { narrow })
        }
    }

    fn install(&self, data: &mut OpenArchiveDataEx) {
        #[cfg(windows)]
        {
            data.arc_name_w = self.wide.as_ptr();
        }
        #[cfg(not(windows))]
        {
            data.arc_name = self.narrow.as_ptr();
        }
    }
}

/// A NUL-terminated wide string from one of UnRAR's buffers.
fn wide_to_string(wide: &[WChar]) -> String {
    let end = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
    decode_wide(&wide[..end])
}

#[cfg(windows)]
fn decode_wide(wide: &[WChar]) -> String {
    String::from_utf16_lossy(wide)
}

#[cfg(not(windows))]
#[allow(clippy::unnecessary_cast)] // wchar_t is i32 on some targets, u32 on others
fn decode_wide(wide: &[WChar]) -> String {
    wide.iter()
        .map(|&c| char::from_u32(c as u32).unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect()
}

/// The `ERAR_*` code's name, for messages and logs.
pub(crate) fn code_name(code: c_int) -> &'static str {
    match code {
        sys::ERAR_SUCCESS => "success",
        sys::ERAR_END_ARCHIVE => "end of archive",
        sys::ERAR_NO_MEMORY => "not enough memory",
        sys::ERAR_BAD_DATA => "bad data (checksum failed)",
        sys::ERAR_BAD_ARCHIVE => "not a valid RAR archive",
        sys::ERAR_UNKNOWN_FORMAT => "unknown archive format",
        sys::ERAR_EOPEN => "could not open the file",
        sys::ERAR_ECREATE => "could not create a file",
        sys::ERAR_ECLOSE => "could not close a file",
        sys::ERAR_EREAD => "read error",
        sys::ERAR_EWRITE => "write error",
        sys::ERAR_SMALL_BUF => "buffer too small",
        sys::ERAR_UNKNOWN => "unknown error",
        sys::ERAR_MISSING_PASSWORD => "password required",
        sys::ERAR_EREFERENCE => "a referenced file is missing",
        sys::ERAR_BAD_PASSWORD => "wrong password",
        ERAR_LARGE_DICT => "the dictionary is larger than this build allows",
        _ => "unexpected error",
    }
}

/// dll.hpp's ERAR_LARGE_DICT (UnRAR 7), newer than unrar_sys's constants.
pub(crate) const ERAR_LARGE_DICT: c_int = 25;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mirrors_match_dll_hpp_on_this_target() {
        // Spelled out once more for the record of this bench (x86-64 Linux:
        // 4-byte wchar_t, 8-byte pointers and LPARAM).
        if size_of::<WChar>() == 4 && POINTER == 8 && size_of::<sys::LPARAM>() == 8 {
            assert_eq!(size_of::<HeaderDataEx>(), 14_340);
            assert_eq!(size_of::<OpenArchiveDataEx>(), 176);
        }
        if size_of::<WChar>() == 2 && POINTER == 8 {
            assert_eq!(size_of::<HeaderDataEx>(), 10_244);
        }
    }

    #[test]
    fn the_dll_is_the_expected_api_version() {
        // SAFETY: a pure query with no arguments.
        assert_eq!(unsafe { sys::RARGetDllVersion() }, 9);
    }

    #[test]
    fn a_missing_archive_fails_to_open() {
        let dir = tempfile::tempdir().unwrap();
        let failure = RarArchive::open(&dir.path().join("Lantern Glow Recolor.rar"), Mode::List)
            .err()
            .unwrap();
        assert_eq!(failure.code, sys::ERAR_EOPEN);
    }

    #[test]
    fn unrar_calls_nonsense_after_a_signature_the_end_of_the_archive() {
        // A characterization of the quirk `rar_frame` exists for: UnRAR opens
        // a valid signature followed by nonsense and then reports "end of
        // archive", indistinguishable from an empty archive. It must never
        // invent an entry; RarSource::open rejects the file before UnRAR
        // lists it.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("not-a-rar.rar");
        std::fs::write(
            &path,
            b"Rar!\x1A\x07\x01\x00 and then nonsense, not headers",
        )
        .unwrap();
        if let Ok(mut archive) = RarArchive::open(&path, Mode::List) {
            let first = archive.read_header();
            assert!(
                !matches!(first, Ok(Some(_))),
                "nonsense listed as {first:?}"
            );
        }
        assert!(crate::rar_frame::check(&path).is_err());
    }

    #[test]
    fn wide_strings_stop_at_the_first_nul() {
        let mut buffer: Vec<WChar> = "Mod/modinfo.ini".chars().map(|c| c as WChar).collect();
        buffer.extend([0, 'x' as WChar]);
        assert_eq!(wide_to_string(&buffer), "Mod/modinfo.ini");
        assert_eq!(wide_to_string(&[]), "");
    }
}
