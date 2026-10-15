//! WASM-only global registry mapping opaque `FileHandle` indices to `web_sys::File`.
//!
//! This allows screens to construct `Cmd` variants with `FileHandle` (which is constructible on host)
//! while the executor resolves handles to real `File` objects at runtime on wasm.

use std::cell::RefCell;
use std::collections::HashMap;

/// An index into the file registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileRegistryIndex(pub u32);

thread_local! {
    static REGISTRY: RefCell<FileRegistry> = RefCell::new(FileRegistry::new());
}

struct FileRegistry {
    files: HashMap<u32, web_sys::File>,
    next_index: u32,
}

impl FileRegistry {
    fn new() -> Self {
        Self {
            files: HashMap::new(),
            next_index: 1,
        }
    }

    fn register(&mut self, file: web_sys::File) -> FileRegistryIndex {
        let index = self.next_index;
        self.next_index = self.next_index.wrapping_add(1);
        // Wrap around protection - if we somehow hit u32::MAX, start from 1 again (practically impossible)
        if self.next_index == 0 {
            self.next_index = 1;
        }
        self.files.insert(index, file);
        FileRegistryIndex(index)
    }

    fn take(&mut self, index: FileRegistryIndex) -> Option<web_sys::File> {
        self.files.remove(&index.0)
    }
}

/// Register a file and return an opaque index. Only available on wasm.
pub fn register_file(file: web_sys::File) -> FileRegistryIndex {
    REGISTRY.with(|r| r.borrow_mut().register(file))
}

/// Take a file from the registry by index, removing it. Only available on wasm.
pub fn take_file(index: FileRegistryIndex) -> Option<web_sys::File> {
    REGISTRY.with(|r| r.borrow_mut().take(index))
}
