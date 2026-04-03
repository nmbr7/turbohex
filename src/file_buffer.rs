//! File loading with automatic strategy selection.
//!
//! Small files (< 1MB) are read entirely into a `Vec<u8>`. Large files are
//! memory-mapped for zero-copy access without consuming heap memory proportional
//! to file size.

use memmap2::Mmap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Files at or above this size (1 MB) are memory-mapped instead of heap-allocated.
const MMAP_THRESHOLD: u64 = 1024 * 1024;

/// A read-only buffer backed by either a heap allocation or a memory-mapped file.
///
/// The choice between strategies is made automatically in [`FileBuffer::open`]
/// based on file size. Both variants expose the same `&[u8]` interface via
/// [`data()`](Self::data).
pub enum FileBuffer {
    /// Heap-allocated buffer for small files (< 1MB).
    Vec(Vec<u8>),
    /// Memory-mapped buffer for large files (>= 1MB).
    Mmap(Mmap),
}

impl FileBuffer {
    /// Opens a file and returns a `FileBuffer` using the appropriate strategy.
    ///
    /// Files smaller than 1MB are read into a `Vec<u8>`. Larger files are
    /// memory-mapped for efficient access.
    ///
    /// # Safety note
    ///
    /// The memory-mapped variant assumes the file is not modified, truncated,
    /// or deleted while mapped. This is acceptable for a read-only hex viewer.
    pub fn open(path: &Path) -> std::io::Result<Self> {
        let file = File::open(path)?;
        let metadata = file.metadata()?;
        let len = metadata.len();

        if len == 0 {
            return Ok(FileBuffer::Vec(Vec::new()));
        }

        if len >= MMAP_THRESHOLD {
            // SAFETY: The file must not be modified, truncated, or deleted while
            // mapped. Doing so can cause SIGBUS or undefined behavior. This is an
            // inherent limitation of memory-mapped I/O and is acceptable for a
            // read-only hex viewer where the file is not expected to change.
            let mmap = unsafe { Mmap::map(&file)? };
            Ok(FileBuffer::Mmap(mmap))
        } else {
            let mut buf = Vec::with_capacity(len as usize);
            let mut file = file;
            file.read_to_end(&mut buf)?;
            Ok(FileBuffer::Vec(buf))
        }
    }

    /// Returns the file contents as a byte slice.
    pub fn data(&self) -> &[u8] {
        match self {
            FileBuffer::Vec(v) => v,
            FileBuffer::Mmap(m) => m,
        }
    }

    /// Returns the file size in bytes.
    pub fn len(&self) -> usize {
        self.data().len()
    }

    /// Returns `true` if the file is empty.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
