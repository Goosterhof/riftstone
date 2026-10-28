//! The mailbox's folders, by handle (BUILD-SPEC § v0.7, the broker's file
//! handling is a boundary of its own).
//!
//! Tamsin can create, replace and link objects in her run's `requests\`, so
//! the stone never follows a path she wrote:
//! - each folder is opened once, by handle, and refused if it is not a plain
//!   folder (a reparse point, a link);
//! - each request is opened relative to that handle by a name the broker
//!   already accepted, without following a reparse point or link, and refused
//!   unread when it is not a regular file. Reads stop at 64 KiB + 1 byte. The
//!   request is removed through the same open file;
//! - each answer is created exclusively as `<id>.tmp` relative to the
//!   `answers\` handle, flushed, and renamed to `<id>.json` without replacing
//!   anything.
//!
//! On Windows (the stone) this is `NtCreateFile` with `RootDirectory` and
//! `FILE_OPEN_REPARSE_POINT`, and a rename by handle. On Linux (the bench)
//! it is `openat` with `O_NOFOLLOW` and `renameat2(RENAME_NOREPLACE)`.

use std::io;
use std::path::Path;

use crate::pawn::mailbox::MAX_REQUEST_BYTES;

/// Why a request was refused unread.
#[derive(Debug)]
pub enum FolderRefusal {
    /// A link or reparse point (a WSL symlink is an `LX_SYMLINK` reparse
    /// point on NTFS). The link itself is removed; its target is never
    /// opened.
    Linked,
    /// A folder, a pipe, a device: anything but a regular file.
    NotRegular,
    /// One byte past 64 KiB.
    TooLarge,
    /// Gone, or locked, or the disk said no.
    Io(io::Error),
}

impl FolderRefusal {
    pub fn sentence(&self) -> String {
        match self {
            Self::Linked => "A request that was a link was removed unread.".into(),
            Self::NotRegular => "A request that was not a plain file was left unread.".into(),
            Self::TooLarge => "A request larger than 64 KiB was removed unread.".into(),
            Self::Io(err) => format!("A request could not be read: {err}."),
        }
    }
}

impl From<io::Error> for FolderRefusal {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

/// Did an answer land, or was the same answer already there?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Written {
    New,
    AlreadyThere,
}

/// One of a run's two mailbox folders, pinned by handle for the run's life.
pub struct MailFolder {
    inner: sys::Folder,
}

impl MailFolder {
    /// Open the folder once, refusing a link or reparse point.
    pub fn open(path: &Path) -> io::Result<Self> {
        Ok(Self {
            inner: sys::Folder::open(path)?,
        })
    }

    /// The names in the folder, in no order.
    pub fn names(&self) -> io::Result<Vec<String>> {
        self.inner.names()
    }

    /// Read one request and remove it: unread when it is a link, not a
    /// regular file, or too large.
    pub fn take(&self, name: &str) -> Result<Vec<u8>, FolderRefusal> {
        self.inner.take(name, MAX_REQUEST_BYTES as u64 + 1)
    }

    /// Write one answer as `name`, never over another file. The same bytes
    /// already under `name` answer [`Written::AlreadyThere`] (a joined or
    /// replayed id); other bytes are an error.
    pub fn write_new(&self, name: &str, bytes: &[u8]) -> io::Result<Written> {
        let tmp = format!("{}.tmp", name.strip_suffix(".json").unwrap_or(name));
        self.inner.write_new(&tmp, name, bytes)
    }
}

#[cfg(target_os = "linux")]
mod sys {
    use std::ffi::{CStr, CString};
    use std::fs::File;
    use std::io::{self, Read, Write};
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;

    use super::{FolderRefusal, Written};

    pub struct Folder {
        fd: OwnedFd,
    }

    fn c(name: &str) -> io::Result<CString> {
        CString::new(name)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "a NUL in a name"))
    }

    fn check(ret: libc::c_int) -> io::Result<libc::c_int> {
        if ret < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(ret)
        }
    }

    impl Folder {
        pub fn open(path: &Path) -> io::Result<Self> {
            let path = CString::new(path.as_os_str().as_bytes())
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "a NUL in a path"))?;
            // SAFETY: a NUL-terminated path; the result is checked.
            let fd = check(unsafe {
                libc::open(
                    path.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            })?;
            // SAFETY: a descriptor just opened and owned by nothing else.
            Ok(Self {
                fd: unsafe { OwnedFd::from_raw_fd(fd) },
            })
        }

        pub fn names(&self) -> io::Result<Vec<String>> {
            // SAFETY: dup of a live descriptor; fdopendir takes the copy.
            let copy =
                check(unsafe { libc::fcntl(self.fd.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 0) })?;
            let dir = unsafe { libc::fdopendir(copy) };
            if dir.is_null() {
                let err = io::Error::last_os_error();
                unsafe { libc::close(copy) };
                return Err(err);
            }
            let mut names = Vec::new();
            // SAFETY: `dir` is a live stream until closedir below; each entry
            // is read before the next readdir call.
            unsafe {
                libc::rewinddir(dir);
                loop {
                    let entry = libc::readdir(dir);
                    if entry.is_null() {
                        break;
                    }
                    let name = CStr::from_ptr((*entry).d_name.as_ptr());
                    if let Ok(name) = name.to_str() {
                        if name != "." && name != ".." {
                            names.push(name.to_string());
                        }
                    }
                }
                libc::closedir(dir);
            }
            Ok(names)
        }

        fn unlink(&self, name: &CStr) {
            // SAFETY: a name relative to the pinned folder; a link entry is
            // removed, never its target.
            unsafe { libc::unlinkat(self.fd.as_raw_fd(), name.as_ptr(), 0) };
        }

        pub fn take(&self, name: &str, cap: u64) -> Result<Vec<u8>, FolderRefusal> {
            let cname = c(name)?;
            // SAFETY: openat relative to the pinned folder, links not followed,
            // a FIFO not waited on.
            let fd = unsafe {
                libc::openat(
                    self.fd.as_raw_fd(),
                    cname.as_ptr(),
                    libc::O_RDONLY
                        | libc::O_NOFOLLOW
                        | libc::O_NONBLOCK
                        | libc::O_NOCTTY
                        | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                let err = io::Error::last_os_error();
                if err.raw_os_error() == Some(libc::ELOOP) {
                    self.unlink(&cname);
                    return Err(FolderRefusal::Linked);
                }
                return Err(FolderRefusal::Io(err));
            }
            // SAFETY: a descriptor just opened.
            let mut file = unsafe { File::from_raw_fd(fd) };
            let meta = file.metadata()?;
            if !meta.file_type().is_file() {
                if !meta.is_dir() {
                    self.unlink(&cname);
                }
                return Err(FolderRefusal::NotRegular);
            }
            let mut bytes = Vec::new();
            (&mut file).take(cap).read_to_end(&mut bytes)?;
            // Remove the entry only if it is still the file that was read.
            let mut now: libc::stat = unsafe { std::mem::zeroed() };
            // SAFETY: fstatat relative to the pinned folder, links not followed.
            let same = unsafe {
                libc::fstatat(
                    self.fd.as_raw_fd(),
                    cname.as_ptr(),
                    &mut now,
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            } == 0
                && {
                    use std::os::unix::fs::MetadataExt;
                    now.st_dev == meta.dev() && now.st_ino == meta.ino()
                };
            if same {
                self.unlink(&cname);
            }
            if bytes.len() as u64 >= cap {
                return Err(FolderRefusal::TooLarge);
            }
            Ok(bytes)
        }

        fn read_existing(&self, name: &CStr) -> io::Result<Vec<u8>> {
            // SAFETY: as in take.
            let fd = check(unsafe {
                libc::openat(
                    self.fd.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
                )
            })?;
            let mut file = unsafe { File::from_raw_fd(fd) };
            let mut bytes = Vec::new();
            (&mut file)
                .take(super::MAX_REQUEST_BYTES as u64 * 16)
                .read_to_end(&mut bytes)?;
            Ok(bytes)
        }

        pub fn write_new(&self, tmp: &str, name: &str, bytes: &[u8]) -> io::Result<Written> {
            let ctmp = c(tmp)?;
            let cname = c(name)?;
            // SAFETY: an exclusive create relative to the pinned folder.
            let fd = check(unsafe {
                libc::openat(
                    self.fd.as_raw_fd(),
                    ctmp.as_ptr(),
                    libc::O_WRONLY
                        | libc::O_CREAT
                        | libc::O_EXCL
                        | libc::O_NOFOLLOW
                        | libc::O_CLOEXEC,
                    0o644,
                )
            })?;
            let mut file = unsafe { File::from_raw_fd(fd) };
            let written = file.write_all(bytes).and_then(|()| file.sync_all());
            drop(file);
            if let Err(err) = written {
                self.unlink(&ctmp);
                return Err(err);
            }
            // SAFETY: both names relative to the pinned folder; nothing replaced.
            let renamed = unsafe {
                libc::renameat2(
                    self.fd.as_raw_fd(),
                    ctmp.as_ptr(),
                    self.fd.as_raw_fd(),
                    cname.as_ptr(),
                    libc::RENAME_NOREPLACE,
                )
            };
            if renamed == 0 {
                return Ok(Written::New);
            }
            let err = io::Error::last_os_error();
            self.unlink(&ctmp);
            if err.kind() == io::ErrorKind::AlreadyExists && self.read_existing(&cname)? == bytes {
                return Ok(Written::AlreadyThere);
            }
            Err(err)
        }
    }
}

#[cfg(windows)]
mod sys {
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};
    use std::path::Path;

    use windows_sys::Wdk::Foundation::OBJECT_ATTRIBUTES;
    use windows_sys::Wdk::Storage::FileSystem::{
        NtCreateFile, FILE_CREATE, FILE_NON_DIRECTORY_FILE, FILE_OPEN, FILE_OPEN_REPARSE_POINT,
        FILE_SYNCHRONOUS_IO_NONALERT,
    };
    use windows_sys::Win32::Foundation::{
        ERROR_NO_MORE_FILES, GENERIC_READ, GENERIC_WRITE, HANDLE, OBJ_DONT_REPARSE, UNICODE_STRING,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FileAttributeTagInfo, FileDispositionInfo, FileFullDirectoryInfo,
        FileFullDirectoryRestartInfo, FileRenameInfo, FlushFileBuffers,
        GetFileInformationByHandleEx, GetFileType, ReadFile, SetFileInformationByHandle, WriteFile,
        DELETE, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_REPARSE_POINT,
        FILE_ATTRIBUTE_TAG_INFO, FILE_DISPOSITION_INFO, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_FULL_DIR_INFO, FILE_LIST_DIRECTORY, FILE_RENAME_INFO,
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_TRAVERSE, FILE_TYPE_DISK,
        OPEN_EXISTING, SYNCHRONIZE,
    };
    use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;

    use super::{FolderRefusal, Written};

    const STATUS_OBJECT_NAME_NOT_FOUND: i32 = 0xC000_0034_u32 as i32;
    const STATUS_OBJECT_NAME_COLLISION: i32 = 0xC000_0035_u32 as i32;

    pub struct Folder {
        handle: OwnedHandle,
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    fn status_error(status: i32) -> io::Error {
        match status {
            STATUS_OBJECT_NAME_NOT_FOUND => io::Error::from(io::ErrorKind::NotFound),
            STATUS_OBJECT_NAME_COLLISION => io::Error::from(io::ErrorKind::AlreadyExists),
            other => io::Error::other(format!("NTSTATUS {:#010x}", other as u32)),
        }
    }

    fn attribute_tag(handle: HANDLE) -> io::Result<u32> {
        let mut info = FILE_ATTRIBUTE_TAG_INFO::default();
        // SAFETY: a live handle and a buffer of the class's own size.
        let ok = unsafe {
            GetFileInformationByHandleEx(
                handle,
                FileAttributeTagInfo,
                (&mut info as *mut FILE_ATTRIBUTE_TAG_INFO).cast(),
                std::mem::size_of::<FILE_ATTRIBUTE_TAG_INFO>() as u32,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(info.FileAttributes)
    }

    /// Mark an open file for deletion through its own handle.
    fn delete_by_handle(handle: HANDLE) -> io::Result<()> {
        let info = FILE_DISPOSITION_INFO { DeleteFile: true };
        // SAFETY: a live handle opened with DELETE access.
        let ok = unsafe {
            SetFileInformationByHandle(
                handle,
                FileDispositionInfo,
                (&info as *const FILE_DISPOSITION_INFO).cast(),
                std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    impl Folder {
        pub fn open(path: &Path) -> io::Result<Self> {
            let mut wpath: Vec<u16> = path.as_os_str().encode_wide().collect();
            wpath.push(0);
            // SAFETY: a NUL-terminated path; the handle is checked.
            let raw = unsafe {
                CreateFileW(
                    wpath.as_ptr(),
                    FILE_LIST_DIRECTORY | FILE_TRAVERSE | SYNCHRONIZE,
                    FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                    std::ptr::null(),
                    OPEN_EXISTING,
                    FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                    std::ptr::null_mut(),
                )
            };
            if raw == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: a handle just opened and owned by nothing else.
            let handle = unsafe { OwnedHandle::from_raw_handle(raw as RawHandle) };
            let attributes = attribute_tag(raw)?;
            if attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
                || attributes & FILE_ATTRIBUTE_DIRECTORY == 0
            {
                return Err(io::Error::other(
                    "the mailbox folder is a link or not a folder",
                ));
            }
            Ok(Self { handle })
        }

        fn raw(&self) -> HANDLE {
            self.handle.as_raw_handle() as HANDLE
        }

        pub fn names(&self) -> io::Result<Vec<String>> {
            // FILE_FULL_DIR_INFO entries are 8-byte aligned.
            let mut buffer = vec![0u64; 8 * 1024];
            let bytes = buffer.len() * 8;
            let mut names = Vec::new();
            let mut class = FileFullDirectoryRestartInfo;
            loop {
                // SAFETY: a live folder handle and a buffer of `bytes` bytes.
                let ok = unsafe {
                    GetFileInformationByHandleEx(
                        self.raw(),
                        class,
                        buffer.as_mut_ptr().cast(),
                        bytes as u32,
                    )
                };
                if ok == 0 {
                    let err = io::Error::last_os_error();
                    if err.raw_os_error() == Some(ERROR_NO_MORE_FILES as i32) {
                        return Ok(names);
                    }
                    return Err(err);
                }
                class = FileFullDirectoryInfo;
                let base = buffer.as_ptr().cast::<u8>();
                let mut offset = 0usize;
                loop {
                    // SAFETY: the system wrote a chain of entries into the
                    // buffer; each offset stays inside it.
                    let entry = unsafe { &*base.add(offset).cast::<FILE_FULL_DIR_INFO>() };
                    let len = entry.FileNameLength as usize / 2;
                    let name = unsafe { std::slice::from_raw_parts(entry.FileName.as_ptr(), len) };
                    let name = String::from_utf16_lossy(name);
                    if name != "." && name != ".." {
                        names.push(name);
                    }
                    if entry.NextEntryOffset == 0 {
                        break;
                    }
                    offset += entry.NextEntryOffset as usize;
                }
            }
        }

        /// `NtCreateFile` relative to the pinned folder.
        fn open_child(
            &self,
            name: &str,
            access: u32,
            disposition: u32,
            options: u32,
        ) -> io::Result<OwnedHandle> {
            let mut wname = wide(name);
            let unicode = UNICODE_STRING {
                Length: (wname.len() * 2) as u16,
                MaximumLength: (wname.len() * 2) as u16,
                Buffer: wname.as_mut_ptr(),
            };
            let attributes = OBJECT_ATTRIBUTES {
                Length: std::mem::size_of::<OBJECT_ATTRIBUTES>() as u32,
                RootDirectory: self.raw(),
                ObjectName: &unicode,
                Attributes: OBJ_DONT_REPARSE,
                SecurityDescriptor: std::ptr::null(),
                SecurityQualityOfService: std::ptr::null(),
            };
            let mut io_status = IO_STATUS_BLOCK::default();
            let mut raw: HANDLE = std::ptr::null_mut();
            // SAFETY: every pointer is to a live local; the handle is checked.
            let status = unsafe {
                NtCreateFile(
                    &mut raw,
                    access | SYNCHRONIZE,
                    &attributes,
                    &mut io_status,
                    std::ptr::null(),
                    FILE_ATTRIBUTE_NORMAL,
                    FILE_SHARE_READ | FILE_SHARE_DELETE,
                    disposition,
                    options | FILE_SYNCHRONOUS_IO_NONALERT | FILE_OPEN_REPARSE_POINT,
                    std::ptr::null(),
                    0,
                )
            };
            if status < 0 {
                return Err(status_error(status));
            }
            // SAFETY: a handle just opened.
            Ok(unsafe { OwnedHandle::from_raw_handle(raw as RawHandle) })
        }

        fn read_all(handle: HANDLE, cap: u64) -> io::Result<Vec<u8>> {
            let mut bytes = Vec::new();
            let mut chunk = vec![0u8; 16 * 1024];
            while (bytes.len() as u64) < cap {
                let want = chunk.len().min((cap - bytes.len() as u64) as usize);
                let mut got = 0u32;
                // SAFETY: a live handle opened for reading; `want` fits the chunk.
                let ok = unsafe {
                    ReadFile(
                        handle,
                        chunk.as_mut_ptr(),
                        want as u32,
                        &mut got,
                        std::ptr::null_mut(),
                    )
                };
                if ok == 0 {
                    return Err(io::Error::last_os_error());
                }
                if got == 0 {
                    break;
                }
                bytes.extend_from_slice(&chunk[..got as usize]);
            }
            Ok(bytes)
        }

        pub fn take(&self, name: &str, cap: u64) -> Result<Vec<u8>, FolderRefusal> {
            let file = self.open_child(name, GENERIC_READ | DELETE, FILE_OPEN, 0)?;
            let raw = file.as_raw_handle() as HANDLE;
            let attributes = attribute_tag(raw)?;
            if attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                // The reparse point itself, opened without following it.
                delete_by_handle(raw)?;
                return Err(FolderRefusal::Linked);
            }
            // SAFETY: a live handle.
            if attributes & FILE_ATTRIBUTE_DIRECTORY != 0
                || unsafe { GetFileType(raw) } != FILE_TYPE_DISK
            {
                return Err(FolderRefusal::NotRegular);
            }
            let bytes = Self::read_all(raw, cap)?;
            delete_by_handle(raw)?;
            if bytes.len() as u64 >= cap {
                return Err(FolderRefusal::TooLarge);
            }
            Ok(bytes)
        }

        pub fn write_new(&self, tmp: &str, name: &str, bytes: &[u8]) -> io::Result<Written> {
            let file = self.open_child(
                tmp,
                GENERIC_WRITE | DELETE,
                FILE_CREATE,
                FILE_NON_DIRECTORY_FILE,
            )?;
            let raw = file.as_raw_handle() as HANDLE;
            let result = (|| {
                let mut done = 0usize;
                while done < bytes.len() {
                    let mut wrote = 0u32;
                    let part = &bytes[done..];
                    // SAFETY: a live handle opened for writing.
                    let ok = unsafe {
                        WriteFile(
                            raw,
                            part.as_ptr(),
                            part.len().min(1 << 20) as u32,
                            &mut wrote,
                            std::ptr::null_mut(),
                        )
                    };
                    if ok == 0 {
                        return Err(io::Error::last_os_error());
                    }
                    done += wrote as usize;
                }
                // SAFETY: a live handle.
                if unsafe { FlushFileBuffers(raw) } == 0 {
                    return Err(io::Error::last_os_error());
                }
                // FILE_RENAME_INFO with the target's name inline, relative to
                // the pinned folder, replacing nothing.
                let target = wide(name);
                let header = std::mem::offset_of!(FILE_RENAME_INFO, FileName);
                let size = header + target.len() * 2 + 2;
                let mut buffer = vec![0u64; size.div_ceil(8)];
                let info = buffer.as_mut_ptr().cast::<FILE_RENAME_INFO>();
                // SAFETY: the buffer holds the header and the name; aligned to 8.
                unsafe {
                    (*info).Anonymous.Flags = 0;
                    (*info).RootDirectory = self.raw();
                    (*info).FileNameLength = (target.len() * 2) as u32;
                    std::ptr::copy_nonoverlapping(
                        target.as_ptr(),
                        (*info).FileName.as_mut_ptr(),
                        target.len(),
                    );
                }
                // SAFETY: a live handle opened with DELETE; `size` bytes.
                if unsafe {
                    SetFileInformationByHandle(raw, FileRenameInfo, info.cast(), size as u32)
                } == 0
                {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            })();
            match result {
                Ok(()) => Ok(Written::New),
                Err(err) => {
                    let _ = delete_by_handle(raw);
                    drop(file);
                    if err.kind() == io::ErrorKind::AlreadyExists {
                        let existing = self.open_child(name, GENERIC_READ, FILE_OPEN, 0)?;
                        let raw = existing.as_raw_handle() as HANDLE;
                        if attribute_tag(raw)? & FILE_ATTRIBUTE_REPARSE_POINT == 0
                            && Self::read_all(raw, bytes.len() as u64 + 1)? == bytes
                        {
                            return Ok(Written::AlreadyThere);
                        }
                    }
                    Err(err)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const ID: &str = "11111111-2222-4333-8444-555555555555";

    fn name() -> String {
        format!("{ID}.json")
    }

    #[test]
    fn a_plain_request_is_read_and_removed() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(name()), b"{\"id\":1}").unwrap();
        let folder = MailFolder::open(dir.path()).unwrap();
        assert_eq!(folder.names().unwrap(), vec![name()]);
        assert_eq!(folder.take(&name()).unwrap(), b"{\"id\":1}");
        assert!(!dir.path().join(name()).exists(), "removed after reading");
        assert!(folder.names().unwrap().is_empty());
    }

    #[test]
    fn one_byte_past_64_kib_is_refused_and_removed() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(name()), vec![b' '; MAX_REQUEST_BYTES]).unwrap();
        let folder = MailFolder::open(dir.path()).unwrap();
        assert_eq!(
            folder.take(&name()).unwrap().len(),
            MAX_REQUEST_BYTES,
            "64 KiB exactly reads"
        );
        fs::write(dir.path().join(name()), vec![b' '; MAX_REQUEST_BYTES + 1]).unwrap();
        assert!(matches!(folder.take(&name()), Err(FolderRefusal::TooLarge)));
        assert!(!dir.path().join(name()).exists());
    }

    #[test]
    fn a_folder_is_left_unread() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(name())).unwrap();
        let folder = MailFolder::open(dir.path()).unwrap();
        assert!(matches!(
            folder.take(&name()),
            Err(FolderRefusal::NotRegular | FolderRefusal::Io(_))
        ));
        assert!(dir.path().join(name()).is_dir());
    }

    #[test]
    fn an_answer_lands_once_and_never_over_another_file() {
        let dir = tempfile::tempdir().unwrap();
        let folder = MailFolder::open(dir.path()).unwrap();
        assert_eq!(folder.write_new(&name(), b"answer").unwrap(), Written::New);
        assert_eq!(fs::read(dir.path().join(name())).unwrap(), b"answer");
        assert_eq!(
            folder.write_new(&name(), b"answer").unwrap(),
            Written::AlreadyThere
        );
        assert!(folder.write_new(&name(), b"other").is_err());
        assert_eq!(fs::read(dir.path().join(name())).unwrap(), b"answer");
        let mut left = folder.names().unwrap();
        left.sort();
        assert_eq!(left, vec![name()], "no .tmp is left behind");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_link_is_removed_unread_and_its_target_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let secret = outside.path().join("canary.txt");
        fs::write(&secret, b"canary").unwrap();
        std::os::unix::fs::symlink(&secret, dir.path().join(name())).unwrap();
        let folder = MailFolder::open(dir.path()).unwrap();
        assert!(matches!(folder.take(&name()), Err(FolderRefusal::Linked)));
        assert!(
            fs::symlink_metadata(dir.path().join(name())).is_err(),
            "the link is gone"
        );
        assert_eq!(
            fs::read(&secret).unwrap(),
            b"canary",
            "its target is untouched"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_pipe_is_refused_without_waiting_on_it() {
        let dir = tempfile::tempdir().unwrap();
        let path =
            std::ffi::CString::new(dir.path().join(name()).as_os_str().as_encoded_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
        let folder = MailFolder::open(dir.path()).unwrap();
        assert!(matches!(
            folder.take(&name()),
            Err(FolderRefusal::NotRegular)
        ));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_linked_mailbox_folder_is_refused() {
        let real = tempfile::tempdir().unwrap();
        let parent = tempfile::tempdir().unwrap();
        let link = parent.path().join("requests");
        std::os::unix::fs::symlink(real.path(), &link).unwrap();
        assert!(MailFolder::open(&link).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn an_answer_is_never_written_through_a_planted_link() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("victim.txt");
        std::os::unix::fs::symlink(&target, dir.path().join(format!("{ID}.tmp"))).unwrap();
        let folder = MailFolder::open(dir.path()).unwrap();
        assert!(folder.write_new(&name(), b"answer").is_err());
        assert!(!target.exists(), "nothing was written through the link");
    }

    #[cfg(windows)]
    #[test]
    fn a_junction_is_removed_unread_and_its_target_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("canary.txt"), b"canary").unwrap();
        let junction = dir.path().join(name());
        let made = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(&junction)
            .arg(outside.path())
            .output()
            .unwrap();
        assert!(made.status.success(), "mklink /J: {made:?}");
        let folder = MailFolder::open(dir.path()).unwrap();
        assert!(matches!(folder.take(&name()), Err(FolderRefusal::Linked)));
        assert!(
            fs::symlink_metadata(&junction).is_err(),
            "the junction is gone"
        );
        assert_eq!(
            fs::read(outside.path().join("canary.txt")).unwrap(),
            b"canary"
        );
    }

    #[cfg(windows)]
    #[test]
    fn a_junctioned_mailbox_folder_is_refused() {
        let real = tempfile::tempdir().unwrap();
        let parent = tempfile::tempdir().unwrap();
        let link = parent.path().join("requests");
        let made = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(&link)
            .arg(real.path())
            .output()
            .unwrap();
        assert!(made.status.success());
        assert!(MailFolder::open(&link).is_err());
    }
}
