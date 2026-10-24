//! Static native-path conversion preserves platform filename identity across COM.
use crate::{INativePath, INativePathAbi, PathEncoding};
use neocogi_com::{ComPtr, ComRef, INcStringAbi, INcStringComPtrExt, NcOutputWriter, NcStatus, NcString, NC_E_INVALID_ARG, com_object};
use std::path::{Path, PathBuf};

/// Typed static client for an immutable native-path capability.
#[derive(Clone)]
pub struct NativePath {
    /// Producer-owned immutable snapshot, independent of the display string's encoding.
    interface: ComPtr<INativePathAbi>,
}
impl NativePath {
    /// Copies a native path into an owned COM value without Unicode conversion of its identity.
    pub fn new(path: &Path) -> Self {
        Self {
            interface: PathValue::from_value(PathValue { path: path.to_owned() })
                .query()
                .expect("path value declares INativePath"),
        }
    }
    /// Retains an already checked immutable capability; native encoding is validated when converting it.
    pub fn from_interface(interface: ComPtr<INativePathAbi>) -> Self {
        Self { interface }
    }
    /// Borrows the capability for a synchronous request without transferring native Rust storage.
    pub fn as_interface(&self) -> ComRef<'_, INativePathAbi> {
        self.interface.as_com_ref()
    }
    /// Transfers this wrapper's proven shared owner without another acquisition.
    pub fn into_interface(self) -> ComPtr<INativePathAbi> {
        self.interface
    }

    /// Copies display-only text; it is unsuitable as a lookup key or reconstructed path.
    pub fn display(&self) -> Result<String, NcStatus> {
        self.interface.display()?.to_string()
    }
    /// Reconstructs the exact host-native path; a foreign platform encoding is an explicit error.
    pub fn to_path_buf(&self) -> Result<PathBuf, NcStatus> {
        decode(self.interface.as_com_ref())
    }
}

pub use neocogi_render_core::platform::path::decode;

#[com_object(interfaces(crate::INativePath))]
/// Ordinary concrete value owns its native path inside whichever binary created the snapshot.
struct PathValue {
    /// Authoritative native identity; display is derived without replacing this value.
    path: PathBuf,
}
impl INativePath for PathValue {
    /// Fixed native encoding for this producer target.
    fn encoding(&self) -> Result<PathEncoding, NcStatus> {
        Ok(if cfg!(windows) { PathEncoding::WindowsUtf16 } else { PathEncoding::UnixBytes })
    }
    /// Counts native units independently of lossy display length.
    fn unit_count(&self) -> Result<u64, NcStatus> {
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            Ok(self.path.as_os_str().encode_wide().count() as u64)
        }
        #[cfg(not(windows))]
        {
            Ok(self.path.as_os_str().as_encoded_bytes().len() as u64)
        }
    }
    /// Rejects a mismatched encoding before inspecting or writing the caller's output.
    fn unix_bytes(&self, output: NcOutputWriter<'_, u8>) -> Result<(), NcStatus> {
        #[cfg(windows)]
        {
            let _ = output;
            Err(NC_E_INVALID_ARG)
        }
        #[cfg(not(windows))]
        {
            copy_range(self.path.as_os_str().as_encoded_bytes(), output)
        }
    }
    /// Windows copies preserve unpaired UTF-16 surrogates rather than passing through a Rust String.
    fn windows_units(&self, output: NcOutputWriter<'_, u16>) -> Result<(), NcStatus> {
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            let units: Vec<u16> = self.path.as_os_str().encode_wide().collect();
            copy_range(&units, output)
        }
        #[cfg(not(windows))]
        {
            let _ = output;
            Err(NC_E_INVALID_ARG)
        }
    }
    /// Produces explicitly lossy display text without modifying the native path identity.
    fn display(&self) -> Result<ComPtr<INcStringAbi>, NcStatus> {
        Ok(NcString::from_utf8_str(&self.path.to_string_lossy()))
    }
}

/// Validates an entire typed range before writing any caller-owned output elements.
fn copy_range<T: neocogi_com::ComInlineValue>(source: &[T], mut output: NcOutputWriter<'_, T>) -> Result<(), NcStatus> {
    let start = usize::try_from(output.offset()).map_err(|_| NC_E_INVALID_ARG)?;
    let end = start.checked_add(output.capacity()).ok_or(NC_E_INVALID_ARG)?;
    let source = source.get(start..end).ok_or(NC_E_INVALID_ARG)?;
    output.extend_from_slice(source)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whole and ranged copies preserve native identity and never partially write an invalid request.
    #[test]
    fn native_path_round_trip_and_range_validation() {
        let source = Path::new("folder/ file name ");
        let path = NativePath::new(source);
        assert_eq!(path.to_path_buf().unwrap(), source);
        assert_eq!(path.display().unwrap(), "folder/ file name ");
        let interface = path.as_interface();
        #[cfg(not(windows))]
        {
            let mut bytes = neocogi_com::NcOutputBuffer::new();
            interface.unix_bytes(bytes.output(1, 4)).unwrap();
            assert_eq!(bytes.as_slice(), b"olde");
            assert_eq!(interface.unix_bytes(bytes.output(u64::MAX, 4)), Err(NC_E_INVALID_ARG));
            assert!(bytes.is_empty());
            let mut units = neocogi_com::NcOutputBuffer::new();
            assert_eq!(interface.windows_units(units.output(0, 2)), Err(NC_E_INVALID_ARG));
            assert!(units.is_empty());
        }
        #[cfg(windows)]
        {
            let mut units = neocogi_com::NcOutputBuffer::new();
            interface.windows_units(units.output(1, 4)).unwrap();
            assert_eq!(units.as_slice(), [111, 108, 100, 101]);
            assert_eq!(interface.windows_units(units.output(u64::MAX, 4)), Err(NC_E_INVALID_ARG));
            assert!(units.is_empty());
        }
        // Oversized capacity is rejected by the shared output preflight before dispatch.
        assert_eq!(
            neocogi_com::NcOutputBuffer::<u16>::new().output(0, usize::MAX).prepare().err(),
            Some(neocogi_com::NC_E_OUT_OF_RANGE)
        );
    }

    /// Different non-Unicode paths may share display text but must round-trip to distinct filesystem identities.
    #[cfg(unix)]
    #[test]
    fn lossy_unix_display_collisions_do_not_change_selection_identity() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};
        let a = PathBuf::from(OsString::from_vec(b"file-\x80".to_vec()));
        let b = PathBuf::from(OsString::from_vec(b"file-\x81".to_vec()));
        let first = NativePath::new(&a);
        let second = NativePath::new(&b);
        assert_eq!(first.display().unwrap(), second.display().unwrap());
        assert_eq!(first.to_path_buf().unwrap(), a);
        assert_eq!(second.to_path_buf().unwrap(), b);
        assert_ne!(first.to_path_buf().unwrap(), second.to_path_buf().unwrap());
    }

    /// Windows native code units preserve unpaired surrogates independently of the display replacement character.
    #[cfg(windows)]
    #[test]
    fn windows_unpaired_surrogates_round_trip_without_utf8_conversion() {
        use std::{ffi::OsString, os::windows::ffi::OsStringExt};
        let native = PathBuf::from(OsString::from_wide(&[102, 0xd800, 103]));
        let path = NativePath::new(&native);
        assert_eq!(path.to_path_buf().unwrap(), native);
        assert_eq!(path.display().unwrap(), "f�g");
    }
}
