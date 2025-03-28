//! RAM filesystem used by [ArceOS](https://github.com/rcore-os/arceos).
//!
//! The implementation is based on [`axfs_vfs`].

#![cfg_attr(not(test), no_std)]
#![cfg_attr(test, feature(noop_waker))]

extern crate alloc;

mod dir;
mod file;
mod interrupts;
#[cfg(test)]
mod tests;

pub use self::dir::DirNode;
pub use self::file::FileNode;
pub use self::interrupts::{Interrupts, INTERRUPT};
use alloc::sync::Arc;
use async_vfs::{VfsNodeOps, VfsNodeRef, VfsOps, VfsResult};
use core::{
    pin::Pin,
    task::{Context, Poll},
};
use spin::once::Once;
/// A RAM filesystem that implements [`axfs_vfs::VfsOps`].
pub struct RamFileSystem {
    parent: Once<VfsNodeRef>,
    root: Arc<DirNode>,
}

impl RamFileSystem {
    /// Create a new instance.
    pub fn new() -> Self {
        Self {
            parent: Once::new(),
            root: DirNode::new(None),
        }
    }

    /// Returns the root directory node in [`Arc<DirNode>`](DirNode).
    pub fn root_dir_node(&self) -> Arc<DirNode> {
        self.root.clone()
    }
}

impl VfsOps for RamFileSystem {
    fn poll_mount(
        self: Pin<&Self>,
        _cx: &mut Context<'_>,
        _path: &str,
        mount_point: &VfsNodeRef,
    ) -> Poll<VfsResult> {
        if let Some(parent) = mount_point.parent() {
            self.root.set_parent(Some(self.parent.call_once(|| parent)));
        } else {
            self.root.set_parent(None);
        }
        Poll::Ready(Ok(()))
    }

    fn root_dir(&self) -> VfsNodeRef {
        self.root.clone()
    }
}

impl Default for RamFileSystem {
    fn default() -> Self {
        Self::new()
    }
}
