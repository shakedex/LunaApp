"""Make a Luna report when OffShoot finishes copying a card.

Attach this script to OffShoot's "File Copy Completed" event
(Settings > Scripting; needs OffShoot Pro and Python 3). OffShoot passes the
event's parameters as one JSON string argument. The event fires once per
destination, and every successful copy gets its own report. Each run opens
its own Luna window.

The card's source info from OffShoot (Shoot Day, Date, Location, ...) goes
into the report header; a saved Luna field with the same name takes
OffShoot's value. Production, company and the rest come from Luna's Settings.

Run with --dump to write the payload OffShoot sends to offshoot_payload.json
next to this script, without running Luna.

Every setting below can also be set through the environment variable of the
same name prefixed with LUNA_ (LUNA_MODE, LUNA_APP, ...).
"""

import json
import os
import subprocess
import sys
from pathlib import Path

# "visible": a Luna window opens and shows the run and its result.
# "headless": no window; luna-cli writes the report and exits.
MODE = "visible"

# Installed locations.
if sys.platform == "darwin":
    LUNA_APP = "/Applications/Luna.app/Contents/MacOS/Luna"
    LUNA_CLI = "/Applications/Luna.app/Contents/MacOS/luna-cli"
else:
    LUNA_APP = r"C:\Program Files\Luna\Luna.exe"
    LUNA_CLI = r"C:\Program Files\Luna\luna-cli.exe"

# Only destinations starting with this path get a report ("" = every destination).
DESTINATION_MATCH = ""

# Where report folders go ("" = Luna's saved output location).
OUTPUT = ""


def setting(name, default):
    return os.environ.get("LUNA_" + name, default)


def value(payload, key):
    """OffShoot names parameters FileCopyCompleted_<key>; accept both forms."""
    v = payload.get("FileCopyCompleted_" + key, payload.get(key))
    return v if isinstance(v, str) else ("" if v is None else str(v))


def source_info(payload):
    """The card's source info as label/value pairs, in OffShoot's order.
    OffShoot sends it as a JSON string; empty values are left out."""
    raw = payload.get("FileCopyCompleted_sourceInfo", payload.get("sourceInfo"))
    if isinstance(raw, str):
        try:
            raw = json.loads(raw or "{}")
        except json.JSONDecodeError:
            return []
    if not isinstance(raw, dict):
        return []
    fields = []
    for label, v in raw.items():
        text = "" if v is None else str(v).strip()
        if str(label).strip() and text:
            fields.append((str(label).strip(), text))
    return fields


def luna_command(payload):
    """The Luna command line for this payload, or None when it should be skipped."""
    if value(payload, "state") != "Success":
        return None
    destination = value(payload, "destinationPath")
    match = setting("DESTINATION_MATCH", DESTINATION_MATCH)
    if not destination or not os.path.normcase(destination).startswith(os.path.normcase(match)):
        return None

    headless = setting("MODE", MODE) == "headless"
    exe = setting("CLI", LUNA_CLI) if headless else setting("APP", LUNA_APP)
    cmd = [exe, "report", destination]
    for label, text in source_info(payload):
        cmd += ["--field", f"{label}={text}"]
    output = setting("OUTPUT", OUTPUT)
    if output:
        cmd += ["--output", output]
    if headless:
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
    # The window outlives this script, so OffShoot isn't kept waiting.
    subprocess.Popen(cmd, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
