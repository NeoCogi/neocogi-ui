//! Private host-native conversion for immutable platform path capabilities.

use neocogi_com::{ComRef, NcCopyToBufferOffset, NcStatus, NC_E_INVALID_ARG};
use neocogi_platform_contracts::{INativePathAbi, PathEncoding};
use std::path::PathBuf;

/// Copies a borrowed platform path into exact host-native owned storage.
pub(crate) fn decode(path: ComRef<'_, INativePathAbi>) -> Result<PathBuf, NcStatus> {
    let encoding = path.encoding()?;
    let count = usize::try_from(path.unit_count()?).map_err(|_| NC_E_INVALID_ARG)?;
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        if encoding != PathEncoding::WindowsUtf16 {
            return Err(NC_E_INVALID_ARG);
        }
        let mut units = copy_buffer::<u16>(count)?;
        path.windows_units(NcCopyToBufferOffset::new(0, &mut units))?;
        Ok(std::ffi::OsString::from_wide(&units).into())
    }
    #[cfg(not(windows))]
    {
        if encoding != PathEncoding::UnixBytes {
            return Err(NC_E_INVALID_ARG);
        }
        let mut bytes = copy_buffer::<u8>(count)?;
        path.unix_bytes(NcCopyToBufferOffset::new(0, &mut bytes))?;
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            Ok(std::ffi::OsString::from_vec(bytes).into())
        }
        #[cfg(not(unix))]
        {
            // Browser targets have no native filesystem picker; a lexical path
            // still must not invent an invalid UTF-8 filesystem identity.
            Ok(String::from_utf8(bytes).map_err(|_| NC_E_INVALID_ARG)?.into())
        }
    }
}

/// Allocates initialized output while preserving allocation failure as a status.
fn copy_buffer<T: Default + Clone>(count: usize) -> Result<Vec<T>, NcStatus> {
    let mut buffer = Vec::new();
    buffer.try_reserve_exact(count).map_err(|_| neocogi_com::NC_E_OUT_OF_MEMORY)?;
    buffer.resize(count, T::default());
    Ok(buffer)
}
