use std::io::{self, Write};
use std::process::ExitCode;

use uda_core::appearance::{AppearanceManager, AppearanceSnapshot};

const HELP: &str = "UDA read-only desktop diagnostics\n\nUsage: uda-cli [appearance] [--json]\n       uda-cli --help\n\nReports theme, accent and appearance capabilities without changing the desktop.\nA valid report may contain unknown, unsupported or error fields.\n";

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Appearance { json: bool },
    Help,
}

fn parse(args: &[String]) -> Result<Command, &'static str> {
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        [] | ["appearance"] => Ok(Command::Appearance { json: false }),
        ["--json"] | ["appearance", "--json"] => Ok(Command::Appearance { json: true }),
        ["--help"] | ["-h"] | ["appearance", "--help"] => Ok(Command::Help),
        _ => Err("unsupported arguments; use --help for read-only commands"),
    }
}

#[cfg(target_os = "linux")]
fn snapshot() -> AppearanceSnapshot {
    uda_platform_linux::appearance::LinuxAppearanceManager::new().snapshot()
}

#[cfg(windows)]
fn snapshot() -> AppearanceSnapshot {
    uda_platform_windows::appearance::WindowsAppearanceManager::new().snapshot()
}

#[cfg(not(any(target_os = "linux", windows)))]
fn snapshot() -> AppearanceSnapshot {
    use uda_core::appearance::AppearanceReading;
    AppearanceSnapshot {
        schema_version: 1,
        capabilities: AppearanceReading::Unsupported,
        theme: AppearanceReading::Unsupported,
        accent: AppearanceReading::Unsupported,
    }
}

fn run() -> Result<(), (u8, String)> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Parse before creating any backend: help and invalid commands touch no OS API.
    let command = parse(&args).map_err(|message| (2, message.to_owned()))?;
    let mut output = io::stdout().lock();
    match command {
        Command::Help => output.write_all(HELP.as_bytes()).map_err(|e| (1, e.to_string()))?,
        Command::Appearance { json } => {
            let report = snapshot();
            if json {
                serde_json::to_writer(&mut output, &report).map_err(|e| (1, e.to_string()))?;
                writeln!(output).map_err(|e| (1, e.to_string()))?;
            } else {
                writeln!(output, "UDA read-only appearance report\n{report:#?}")
                    .map_err(|e| (1, e.to_string()))?;
            }
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err((code, message)) => {
            eprintln!("uda-cli: {message}");
            ExitCode::from(code)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn defaults_and_json_select_only_read_only_appearance() {
        assert_eq!(parse(&[]), Ok(Command::Appearance { json: false }));
        assert_eq!(parse(&args(&["appearance"])), Ok(Command::Appearance { json: false }));
        assert_eq!(parse(&args(&["--json"])), Ok(Command::Appearance { json: true }));
        assert_eq!(parse(&args(&["appearance", "--json"])), Ok(Command::Appearance { json: true }));
    }

    #[test]
    fn help_is_available_without_a_desktop() {
        for values in [&["--help"][..], &["-h"], &["appearance", "--help"]] {
            assert_eq!(parse(&args(values)), Ok(Command::Help));
        }
    }

    #[test]
    fn mutating_and_ambiguous_commands_are_rejected() {
        for values in [&["set-wallpaper", "/private.jpg"][..], &["shutdown"], &["--json", "--json"], &["appearance", "--json", "extra"]] {
            assert!(parse(&args(values)).is_err());
        }
    }
}
