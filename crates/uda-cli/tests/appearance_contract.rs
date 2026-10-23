use std::process::Command;

use serde_json::json;
use uda_core::appearance::{
    AppearanceCapabilities, AppearanceErrorKind, AppearanceManager, AppearanceReading,
    AppearanceSnapshot,
};
use uda_core::capability::{Capability, RgbaColor, Theme};
use uda_core::error::UdaError;

#[test]
fn json_contract_keeps_unknown_and_alpha_explicit() -> Result<(), Box<dyn std::error::Error>> {
    let report = AppearanceSnapshot {
        schema_version: 1,
        capabilities: AppearanceReading::Available {
            value: AppearanceCapabilities {
                detect_theme: true,
                read_accent_color: true,
                follow_system_theme: false,
            },
        },
        theme: AppearanceReading::Unknown,
        accent: AppearanceReading::Available {
            value: RgbaColor {
                r: 17,
                g: 34,
                b: 51,
                a: 127,
            },
        },
    };
    assert_eq!(
        serde_json::to_value(report)?,
        json!({
            "schema_version": 1,
            "capabilities": {"status": "available", "value": {
                "detect_theme": true, "read_accent_color": true, "follow_system_theme": false
            }},
            "theme": {"status": "unknown"},
            "accent": {"status": "available", "value": {"r": 17, "g": 34, "b": 51, "a": 127}}
        })
    );
    for (theme, value) in [(Theme::Dark, "dark"), (Theme::Light, "light"), (Theme::Auto, "auto")] {
        assert_eq!(serde_json::to_value(theme)?, json!(value));
    }
    Ok(())
}

struct Degraded;

impl AppearanceManager for Degraded {
    fn capabilities(&self) -> Result<Capability, UdaError> {
        Err(UdaError::Internal("SECRET_CAPABILITY_CANARY".into()))
    }
    fn detect_theme(&self) -> Result<Theme, UdaError> {
        Ok(Theme::Light)
    }
    fn get_accent_color(&self) -> Result<RgbaColor, UdaError> {
        Err(UdaError::DetectionFailed("SECRET_ACCENT_CANARY".into()))
    }
}

#[test]
fn serialization_redacts_errors_without_discarding_good_fields() -> Result<(), Box<dyn std::error::Error>> {
    let report = Degraded.snapshot();
    let encoded = serde_json::to_value(report)?;
    assert_eq!(encoded["theme"], json!({"status": "available", "value": "light"}));
    assert_eq!(encoded["capabilities"], json!({"status": "error", "kind": "internal"}));
    assert_eq!(encoded["accent"], json!({"status": "error", "kind": "detection"}));
    assert!(!encoded.to_string().contains("SECRET_"));
    Ok(())
}

#[test]
fn unsupported_and_error_codes_have_no_implicit_values() -> Result<(), Box<dyn std::error::Error>> {
    let unsupported: AppearanceReading<Theme> = AppearanceReading::Unsupported;
    assert_eq!(serde_json::to_value(unsupported)?, json!({"status": "unsupported"}));
    for (kind, name) in [
        (AppearanceErrorKind::Detection, "detection"),
        (AppearanceErrorKind::Command, "command"),
        (AppearanceErrorKind::Io, "io"),
        (AppearanceErrorKind::Internal, "internal"),
    ] {
        let error: AppearanceReading<Theme> = AppearanceReading::Error { kind };
        assert_eq!(serde_json::to_value(error)?, json!({"status": "error", "kind": name}));
    }
    Ok(())
}

#[test]
fn executable_help_and_rejected_commands_work_without_a_session() -> Result<(), Box<dyn std::error::Error>> {
    let executable = env!("CARGO_BIN_EXE_uda-cli");
    let help = Command::new(executable)
        .arg("--help")
        .env("DBUS_SESSION_BUS_ADDRESS", "unix:path=/uda-test-no-session")
        .output()?;
    assert!(help.status.success());
    assert!(String::from_utf8(help.stdout)?.contains("read-only"));
    let invalid = Command::new(executable)
        .args(["set-wallpaper", "PRIVATE_ARGUMENT_CANARY"])
        .env("DBUS_SESSION_BUS_ADDRESS", "unix:path=/uda-test-no-session")
        .output()?;
    assert_eq!(invalid.status.code(), Some(2));
    assert!(invalid.stdout.is_empty());
    assert!(!String::from_utf8(invalid.stderr)?.contains("PRIVATE_ARGUMENT_CANARY"));
    Ok(())
}
