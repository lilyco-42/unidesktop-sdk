use crate::capability::{Capability, RgbaColor, Theme};
use crate::error::UdaError;

mod snapshot;
pub use snapshot::{
    AppearanceCapabilities, AppearanceErrorKind, AppearanceReading, AppearanceSnapshot,
};

/// Cross-platform system appearance management interface.
pub trait AppearanceManager {
    /// Detect the current system theme preference (dark / light / auto).
    fn detect_theme(&self) -> Result<Theme, UdaError>;

    /// Read the system accent color.
    ///
    /// Returns `UdaError::NotSupported` if the current platform does not expose an
    /// accent color, or if the value cannot be parsed.
    fn get_accent_color(&self) -> Result<RgbaColor, UdaError>;

    /// Return the set of capabilities supported by the current backend.
    fn capabilities(&self) -> Result<Capability, UdaError>;

    /// Collect independent, read-only appearance observations.
    ///
    /// This is not an atomic OS snapshot: preferences may change between reads.
    /// Missing capabilities are not probed. Capability-query errors do not hide
    /// successful reads. Raw backend error text is excluded from the report.
    fn snapshot(&self) -> AppearanceSnapshot {
        AppearanceSnapshot::read(self)
    }
}
