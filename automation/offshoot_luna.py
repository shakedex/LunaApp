"""Make a Luna report when OffShoot finishes a transfer.

Attach this script to OffShoot's "File Copy Completed" event
(Settings > Scripting; needs OffShoot Pro). OffShoot passes the event's
parameters as one JSON string argument. The event fires once per
destination, so set DESTINATION_MATCH to the drive or folder whose copy
Luna should report on; the other copies are skipped.

Run with --dump to write the payload OffShoot sends to offshoot_payload.json
next to this script, without running Luna.

Every setting below can also be set through the environment variable of the
same name prefixed with LUNA_ (LUNA_MODE, LUNA_CLI, ...).
"""

import json
import os
import subprocess
import sys
from pathlib import Path

# "headless": no window, Luna writes the report and exits.
# "visible": the Luna window opens and shows the run.
MODE = "headless"

# Installed locations (Windows default; macOS: /Applications/Luna.app/Contents/MacOS/...).
LUNA_CLI = r"C:\Program Files\Luna\luna-cli.exe"
LUNA_APP = r"C:\Program Files\Luna\Luna.exe"

# Only destinations starting with this path get a report ("" = every destination).
DESTINATION_MATCH = ""

# Where report folders go ("" = Luna's saved output location).
OUTPUT = ""


def setting(name, default):
    return os.environ.get("LUNA_" + name, default)


def value(payload, key):
    """OffShoot's docs name parameters FileCopyCompleted_<key>; accept both forms."""
    v = payload.get("FileCopyCompleted_" + key, payload.get(key))
    return v if isinstance(v, str) else ("" if v is None else str(v))


def luna_command(payload):
    """The Luna command line for this payload, or None when it should be skipped."""
    if value(payload, "state") != "Success":
        return None
    destination = value(payload, "destinationPath")
    match = setting("DESTINATION_MATCH", DESTINATION_MATCH)
    if not destination or not os.path.normcase(destination).startswith(os.path.normcase(match)):
        return None

    mode = setting("MODE", MODE)
    exe = setting("CLI", LUNA_CLI) if mode == "headless" else setting("APP", LUNA_APP)
    cmd = [exe, "report", destination]
    preset = value(payload, "presetName")
    if preset:
        cmd += ["--production", preset]
    try:
        info = json.loads(value(payload, "sourceInfo") or "{}")
    except json.JSONDecodeError:
        info = {}
    if isinstance(info, dict):
        for label in ("Shoot Day", "Source Name", "Date"):
            if info.get(label):
                cmd += ["--field", f"{label}={info[label]}"]
    output = setting("OUTPUT", OUTPUT)
    if output:
        cmd += ["--output", output]
    if mode == "headless":
        cmd.append("--json")
    return cmd


def main(argv):
    if len(argv) < 2:
        print("usage: offshoot_luna.py '<OffShoot JSON payload>' [--dump]", file=sys.stderr)
        return 2
    try:
        payload = json.loads(argv[1])
    except json.JSONDecodeError as e:
        print(f"The payload isn't JSON: {e}", file=sys.stderr)
        return 2
    if "--dump" in argv[2:]:
        Path(__file__).with_name("offshoot_payload.json").write_text(
            json.dumps(payload, indent=2), encoding="utf-8"
        )
        return 0

    cmd = luna_command(payload)
    if cmd is None:
        return 0
    # A list, not a string: paths ending in a backslash stay intact on Windows.
    if setting("MODE", MODE) == "headless":
        return subprocess.run(cmd).returncode
    subprocess.Popen(cmd)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
