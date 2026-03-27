use memmap2::Mmap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

const MMAP_THRESHOLD: u64 = 1024 * 1024; // 1MB

pub enum FileBuffer {
    Vec(Vec<u8>),
    Mmap(Mmap),
}

impl FileBuffer {
    pub fn open(path: &Path) -> std::io::Result<Self> {
        let file = File::open(path)?;
        let metadata = file.metadata()?;
        let len = metadata.len();

        if len == 0 {
            return Ok(FileBuffer::Vec(Vec::new()));
        }

        if len >= MMAP_THRESHOLD {
            let mmap = unsafe { Mmap::map(&file)? };
            Ok(FileBuffer::Mmap(mmap))
        } else {
            let mut buf = Vec::with_capacity(len as usize);
            let mut file = file;
            file.read_to_end(&mut buf)?;
            Ok(FileBuffer::Vec(buf))
        }
    }

    pub fn data(&self) -> &[u8] {
        match self {
            FileBuffer::Vec(v) => v,
            FileBuffer::Mmap(m) => m,
        }
    }

    pub fn len(&self) -> usize {
        self.data().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
