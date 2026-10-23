"""Read-only integration checks; run the built CLI only in GitHub Actions."""

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def invoke(executable, environment):
    result = subprocess.run(
        [str(executable), "appearance", "--json"],
        env=environment, capture_output=True, text=True, timeout=20, check=False,
    )
    require(result.returncode == 0, "CLI did not produce a report")
    require(not result.stderr, "unexpected diagnostic stderr")
    require("UDA_SECRET_CANARY_7d2a" not in result.stdout, "environment secret echoed")
    report = json.loads(result.stdout)
    require(set(report) == {"schema_version", "capabilities", "theme", "accent"}, "unexpected top-level fields")
    require(report["schema_version"] == 1, "unknown schema version")
    for name in ("capabilities", "theme", "accent"):
        reading = report[name]
        status = reading["status"]
        if status == "available":
            require(set(reading) == {"status", "value"}, "unexpected reading fields")
            value = reading["value"]
            if name == "theme":
                require(value in ("dark", "light", "auto"), "unknown theme disguised as a value")
            elif name == "accent":
                require(set(value) == {"r", "g", "b", "a"}, "RGBA contract mismatch")
                require(all(type(channel) is int and 0 <= channel <= 255 for channel in value.values()), "invalid RGBA channels")
            else:
                require(set(value) == {"detect_theme", "read_accent_color", "follow_system_theme"}, "unexpected capability fields")
                require(all(type(flag) is bool for flag in value.values()), "invalid capability flags")
        elif status in ("unknown", "unsupported"):
            require(set(reading) == {"status"}, "unavailable field has an implicit value")
        elif status == "error":
            require(set(reading) == {"status", "kind"}, "raw error text or fields exposed")
            require(reading["kind"] in ("detection", "command", "io", "internal"), "unknown error code")
        else:
            raise AssertionError("unknown reading status")
    return report


def windows_values():
    import winreg

    def read(subkey, name):
        try:
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, subkey, 0, winreg.KEY_READ) as key:
                value, kind = winreg.QueryValueEx(key, name)
                return value if kind == winreg.REG_DWORD else None
        except FileNotFoundError:
            return None

    key = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"
    return (
        read(key, "AppsUseLightTheme"), read(key, "SystemUsesLightTheme"),
        read(r"Software\Microsoft\Windows\DWM", "AccentColor"),
    )


def check_windows(executable, environment):
    before = windows_values()
    report = invoke(executable, environment)
    require(windows_values() == before, "appearance changed during read-only observation")
    flag = next((value for value in before[:2] if value in (0, 1)), None)
    expected_theme = {"status": "unsupported"} if flag is None else {
        "status": "available", "value": "dark" if flag == 0 else "light",
    }
    require(report["theme"] == expected_theme, "theme differs from independent registry read")
    packed = before[2]
    expected_accent = {"status": "unsupported"} if packed is None else {
        "status": "available", "value": {
            "r": packed & 255, "g": (packed >> 8) & 255,
            "b": (packed >> 16) & 255, "a": (packed >> 24) & 255,
        },
    }
    require(report["accent"] == expected_accent, "accent differs from independent registry read")
    print("PASS: native Windows reads agree; observed registry values unchanged")


def check_linux(executable, environment):
    # Isolate from the real desktop and session bus. The only CLI backend is
    # this fixture: any command except --version and the expected reads fails.
    with tempfile.TemporaryDirectory(prefix="uda-read-only-") as directory:
        root = Path(directory)
        log = root / "calls"
        gsettings = root / "gsettings"
        gsettings.write_text(
            '#!/bin/sh\nprintf "%s\\n" "$*" >> "$UDA_TEST_CALLS"\n'
            'case "$*" in\n'
            '  --version) echo 1;;\n'
            '  "get org.gnome.desktop.interface color-scheme") echo "prefer-dark";;\n'
            '  "get org.gnome.desktop.interface accent-color") echo "rgba(17,34,51,0.5)";;\n'
            '  *) exit 91;;\nesac\n', encoding="utf-8",
        )
        gsettings.chmod(0o700)
        fixture = dict(environment, PATH=str(root), UDA_TEST_CALLS=str(log),
                       DBUS_SESSION_BUS_ADDRESS="unix:path=/uda-test-no-session")
        report = invoke(executable, fixture)
        require(report["theme"] == {"status": "available", "value": "dark"}, "Linux read fallback not used")
        require(report["accent"] == {"status": "available", "value": {"r": 17, "g": 34, "b": 51, "a": 128}}, "Linux RGBA fallback mismatch")
        calls = log.read_text(encoding="utf-8").splitlines()
        require(calls == ["--version", "get org.gnome.desktop.interface color-scheme", "get org.gnome.desktop.interface accent-color"], "unexpected CLI backend command")
        gsettings.unlink()
        unavailable = invoke(executable, fixture)
        require(unavailable["theme"] == {"status": "unknown"}, "headless theme must remain unknown")
        require(unavailable["accent"] == {"status": "unsupported"}, "missing accent backend not explicit")
        print("PASS: Linux fallback reads only; missing desktop degrades without panic")


def main():
    require(os.environ.get("GITHUB_ACTIONS") == "true", "run these integration checks in Actions only")
    executable = Path(sys.argv[1]).resolve(strict=True)
    environment = dict(os.environ, UDA_TEST_SECRET="UDA_SECRET_CANARY_7d2a")
    if sys.platform == "win32":
        check_windows(executable, environment)
    elif sys.platform == "linux":
        check_linux(executable, environment)
    else:
        raise RuntimeError("unsupported integration-test host")


if __name__ == "__main__":
    main()
