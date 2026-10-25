//! Somewhere to write a file whole before it takes the place of whatever is at
//! its path. DESIGN.md §7.
//!
//! On Linux and Windows the file is written beside the destination under a
//! hidden name and renamed over it. macOS cannot: under the App Sandbox the
//! grant a person gives by choosing a file in the save panel covers that file
//! and not its directory, so a sibling cannot be created. There a destination
//! that does not exist yet is written in place, and one that does is written
//! in the directory macOS provides for replacing it, on its own volume, and
//! put in place with `replaceItemAtURL`. This is segler's and
//! slipcase-desktop's arrangement; the bindings are safe functions, so the
//! crate stays `forbid(unsafe_code)`.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::io;
use std::path::{Path, PathBuf};

/// A file being written for `destination`, removed again unless finished.
pub(crate) struct Staging {
    path: PathBuf,
    place: Place,
    finished: bool,
}

enum Place {
    /// Written beside the destination under a hidden name, and renamed onto it.
    #[cfg(not(target_os = "macos"))]
    Beside(PathBuf),
    /// Written at the destination itself, which did not exist.
    #[cfg(target_os = "macos")]
    InPlace,
    /// Written in the scratch directory, held until the replacement is over,
    /// and replaced onto the original.
    #[cfg(target_os = "macos")]
    Replacing { _scratch: macos::Scratch, original: PathBuf },
}

impl Staging {
    #[cfg_attr(not(target_os = "macos"), allow(clippy::unnecessary_wraps))]
    pub(crate) fn for_destination(destination: &Path) -> io::Result<Self> {
        #[cfg(target_os = "macos")]
        {
            if !destination.exists() {
                return Ok(Self { path: destination.to_owned(), place: Place::InPlace, finished: false });
            }
            let original = std::fs::canonicalize(destination)?;
            let scratch = macos::Scratch::on_the_volume_holding(&original)?;
            let path = scratch.path().join(original.file_name().unwrap_or_default());
            Ok(Self { path, place: Place::Replacing { _scratch: scratch, original }, finished: false })
        }
        #[cfg(not(target_os = "macos"))]
        {
            let name = destination.file_name().map_or_else(|| "export".into(), |n| n.to_string_lossy().into_owned());
            let path = destination.with_file_name(format!(".{name}.part"));
            Ok(Self { path, place: Place::Beside(destination.to_owned()), finished: false })
        }
    }

    /// Where to write.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// Put what was written in the destination's place.
    pub(crate) fn finish(mut self) -> io::Result<()> {
        match &self.place {
            #[cfg(not(target_os = "macos"))]
            Place::Beside(destination) => std::fs::rename(&self.path, destination)?,
            #[cfg(target_os = "macos")]
            Place::InPlace => {}
            #[cfg(target_os = "macos")]
            Place::Replacing { original, .. } => macos::replace(original, &self.path)?,
        }
        self.finished = true;
        Ok(())
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        if !self.finished {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::io;
    use std::path::{Path, PathBuf};

    use objc2::rc::Retained;
    use objc2_foundation::{
        NSFileManager, NSFileManagerItemReplacementOptions, NSSearchPathDirectory, NSSearchPathDomainMask, NSString,
        NSURL,
    };

    /// The directory macOS made for one replacement, removed when it is over.
    pub(super) struct Scratch(PathBuf);

    impl Scratch {
        /// On the volume holding `original`: a replacement across volumes is
        /// refused with EXDEV.
        pub(super) fn on_the_volume_holding(original: &Path) -> io::Result<Self> {
            let url = NSFileManager::defaultManager()
                .URLForDirectory_inDomain_appropriateForURL_create_error(
                    NSSearchPathDirectory::ItemReplacementDirectory,
                    NSSearchPathDomainMask::UserDomainMask,
                    Some(&url_for(original)),
                    true,
                )
                .map_err(|e| io::Error::other(format!("nowhere to write {}: {}", original.display(), e.localizedDescription())))?;
            let path = url.path().ok_or_else(|| io::Error::other("macOS named a replacement directory with no path"))?;
            Ok(Self(PathBuf::from(path.to_string())))
        }

        pub(super) fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Replace `original` with `staged`, keeping the original's metadata.
    pub(super) fn replace(original: &Path, staged: &Path) -> io::Result<()> {
        NSFileManager::defaultManager()
            .replaceItemAtURL_withItemAtURL_backupItemName_options_resultingItemURL_error(
                &url_for(original),
                &url_for(staged),
                None,
                NSFileManagerItemReplacementOptions::empty(),
                None,
            )
            .map(|_| ())
            .map_err(|e| io::Error::other(format!("cannot replace {}: {}", original.display(), e.localizedDescription())))
    }

    fn url_for(path: &Path) -> Retained<NSURL> {
        NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()))
    }
}
