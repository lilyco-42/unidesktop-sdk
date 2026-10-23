use serde::Serialize;

use super::AppearanceManager;
use crate::capability::{Capability, RgbaColor, Theme};
use crate::error::UdaError;

/// Versioned, read-only observations for host applications.
///
/// A collected report does not mean all fields are supported. Inspect each
/// status. No identity, credentials, wallpaper paths or network data is read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppearanceSnapshot {
    pub schema_version: u32,
    pub capabilities: AppearanceReading<AppearanceCapabilities>,
    pub theme: AppearanceReading<Theme>,
    pub accent: AppearanceReading<RgbaColor>,
}

/// Appearance capabilities; unrelated system/power flags are excluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AppearanceCapabilities {
    pub detect_theme: bool,
    pub read_accent_color: bool,
    pub follow_system_theme: bool,
}

/// Unknown preferences, unsupported operations and failed reads differ.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AppearanceReading<T> {
    Available { value: T },
    Unknown,
    Unsupported,
    Error { kind: AppearanceErrorKind },
}

/// Bounded categories deliberately excluding raw backend error messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AppearanceErrorKind {
    Detection,
    Command,
    Io,
    Internal,
}

impl<T> AppearanceReading<T> {
    fn from_result(result: Result<T, UdaError>) -> Self {
        match result {
            Ok(value) => Self::Available { value },
            Err(UdaError::NotSupported(_)) => Self::Unsupported,
            Err(UdaError::DetectionFailed(_)) => Self::Error {
                kind: AppearanceErrorKind::Detection,
            },
            Err(UdaError::CommandFailed(_)) => Self::Error {
                kind: AppearanceErrorKind::Command,
            },
            Err(UdaError::Io(_)) => Self::Error {
                kind: AppearanceErrorKind::Io,
            },
            Err(UdaError::Internal(_)) => Self::Error {
                kind: AppearanceErrorKind::Internal,
            },
        }
    }
}

impl AppearanceSnapshot {
    fn read_field<T>(
        flags: Option<Capability>,
        required: Capability,
        read: impl FnOnce() -> Result<T, UdaError>,
    ) -> AppearanceReading<T> {
        if flags.is_some_and(|flags| !flags.contains(required)) {
            AppearanceReading::Unsupported
        } else {
            AppearanceReading::from_result(read())
        }
    }

    pub(super) fn read(manager: &(impl AppearanceManager + ?Sized)) -> Self {
        let capabilities = manager.capabilities();
        let flags = capabilities.as_ref().ok().copied();
        let theme =
            match Self::read_field(flags, Capability::DETECT_THEME, || manager.detect_theme()) {
                AppearanceReading::Available {
                    value: Theme::Unknown,
                } => AppearanceReading::Unknown,
                reading => reading,
            };
        let accent = Self::read_field(flags, Capability::READ_ACCENT_COLOR, || {
            manager.get_accent_color()
        });
        let capabilities =
            AppearanceReading::from_result(capabilities.map(|flags| AppearanceCapabilities {
                detect_theme: flags.contains(Capability::DETECT_THEME),
                read_accent_color: flags.contains(Capability::READ_ACCENT_COLOR),
                follow_system_theme: flags.contains(Capability::FOLLOW_SYSTEM_THEME),
            }));
        Self {
            schema_version: 1,
            capabilities,
            theme,
            accent,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct Manager {
        flags: Result<Capability, fn() -> UdaError>,
        theme: Result<Theme, fn() -> UdaError>,
        accent: Result<RgbaColor, fn() -> UdaError>,
        reads: Cell<(usize, usize)>,
    }

    impl AppearanceManager for Manager {
        fn capabilities(&self) -> Result<Capability, UdaError> {
            self.flags.map_err(|error| error())
        }
        fn detect_theme(&self) -> Result<Theme, UdaError> {
            let (theme, accent) = self.reads.get();
            self.reads.set((theme + 1, accent));
            self.theme.map_err(|error| error())
        }
        fn get_accent_color(&self) -> Result<RgbaColor, UdaError> {
            let (theme, accent) = self.reads.get();
            self.reads.set((theme, accent + 1));
            self.accent.map_err(|error| error())
        }
    }

    fn manager() -> Manager {
        Manager {
            flags: Ok(Capability::DETECT_THEME | Capability::READ_ACCENT_COLOR),
            theme: Ok(Theme::Dark),
            accent: Ok(RgbaColor {
                r: 17,
                g: 34,
                b: 51,
                a: 127,
            }),
            reads: Cell::new((0, 0)),
        }
    }

    #[test]
    fn reads_each_available_field_once_without_changing_values() {
        let manager = manager();
        let object: &dyn AppearanceManager = &manager;
        let report = object.snapshot();
        assert_eq!(report.schema_version, 1);
        assert_eq!(
            report.theme,
            AppearanceReading::Available { value: Theme::Dark }
        );
        assert_eq!(
            report.accent,
            AppearanceReading::Available {
                value: RgbaColor {
                    r: 17,
                    g: 34,
                    b: 51,
                    a: 127
                }
            }
        );
        assert_eq!(manager.reads.get(), (1, 1));
    }

    #[test]
    fn unknown_is_not_light_or_auto() {
        let mut manager = manager();
        manager.theme = Ok(Theme::Unknown);
        assert_eq!(manager.snapshot().theme, AppearanceReading::Unknown);
        manager.theme = Ok(Theme::Auto);
        assert_eq!(
            manager.snapshot().theme,
            AppearanceReading::Available { value: Theme::Auto }
        );
    }

    #[test]
    fn missing_capabilities_skip_backend_reads() {
        let mut manager = manager();
        manager.flags = Ok(Capability::empty());
        let report = manager.snapshot();
        assert_eq!(manager.reads.get(), (0, 0));
        assert_eq!(report.theme, AppearanceReading::Unsupported);
        assert_eq!(report.accent, AppearanceReading::Unsupported);
    }

    #[test]
    fn capability_failure_does_not_hide_successful_reads() {
        let mut manager = manager();
        manager.flags = Err(|| UdaError::DetectionFailed("private bus address".into()));
        let report = manager.snapshot();
        assert_eq!(
            report.capabilities,
            AppearanceReading::Error {
                kind: AppearanceErrorKind::Detection
            }
        );
        assert_eq!(
            report.theme,
            AppearanceReading::Available { value: Theme::Dark }
        );
        assert!(matches!(report.accent, AppearanceReading::Available { .. }));
    }

    #[test]
    fn field_errors_do_not_poison_other_reads_or_echo_messages() {
        let mut manager = manager();
        manager.theme = Err(|| UdaError::CommandFailed("sensitive path or credential".into()));
        let report = manager.snapshot();
        assert_eq!(
            report.theme,
            AppearanceReading::Error {
                kind: AppearanceErrorKind::Command
            }
        );
        assert!(matches!(report.accent, AppearanceReading::Available { .. }));
        assert!(!format!("{report:?}").contains("sensitive"));
    }

    #[test]
    fn unsupported_accent_remains_explicit() {
        let mut manager = manager();
        manager.accent = Err(|| UdaError::NotSupported("no accent backend".into()));
        assert_eq!(manager.snapshot().accent, AppearanceReading::Unsupported);
    }

    #[test]
    fn error_categories_preserve_io_and_internal_failures() {
        let io: AppearanceReading<()> =
            AppearanceReading::from_result(Err(UdaError::Io(std::io::Error::other("private"))));
        let internal: AppearanceReading<()> =
            AppearanceReading::from_result(Err(UdaError::Internal("private".into())));
        assert_eq!(
            io,
            AppearanceReading::Error {
                kind: AppearanceErrorKind::Io
            }
        );
        assert_eq!(
            internal,
            AppearanceReading::Error {
                kind: AppearanceErrorKind::Internal
            }
        );
    }
}
