//! Native filesystem identity remains distinct from potentially lossy UTF-8 display text.
use neocogi_com::{ComPtr, INcStringAbi, NcCopyToBufferOffset, NcStatus, com_interface, com_int_enum};

com_int_enum! {
    /// Native path representation carried by an immutable path capability.
    pub enum PathEncoding: u8 {
        /// Exact Unix filename bytes; invalid UTF-8 is preserved.
        UNIX_BYTES = 0,
        /// Exact Windows UTF-16 code units; unpaired surrogates are preserved.
        WINDOWS_UTF16 = 1,
    }
}
#[com_interface(auto_guid = "microui.NativePath:v1")]
/// Immutable native path snapshot with explicit encoding and separate display-only text.
pub trait INativePath {
    /// Reports which typed copy method applies; callers reject incompatible host encodings.
    fn encoding(&self) -> Result<PathEncoding, NcStatus>;
    /// Returns bytes or UTF-16 code units according to encoding, without a trailing terminator.
    fn unit_count(&self) -> Result<u64, NcStatus>;
    /// Copies the exact requested byte range; wrong encoding or invalid range changes no output.
    fn unix_bytes(&self, output: NcCopyToBufferOffset<'_, u8>) -> Result<(), NcStatus>;
    /// Copies the exact requested UTF-16 range; wrong encoding or invalid range changes no output.
    fn windows_units(&self, output: NcCopyToBufferOffset<'_, u16>) -> Result<(), NcStatus>;
    /// Returns human-readable UTF-8 which must never be used to reconstruct native selection.
    fn display(&self) -> Result<ComPtr<INcStringAbi>, NcStatus>;
}
