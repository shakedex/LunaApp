"""Tests for offshoot_luna.py: python -m unittest discover automation"""

import json
import os
import unittest

import offshoot_luna as script


def payload(**over):
    p = {
        "FileCopyCompleted_state": "Success",
        "FileCopyCompleted_destinationPath": "/Volumes/RAID 01/Project A/Media/20260521/A001-01",
        "FileCopyCompleted_presetName": "A cam preset",
        "FileCopyCompleted_sourceInfo": json.dumps(
            {"Date": "2026-05-21", "Shoot Day": "01", "Source Name": "A001", "Location": ""}
        ),
    }
    p.update(over)
    return p


class LunaCommand(unittest.TestCase):
    def setUp(self):
        for name in ("MODE", "APP", "CLI", "DESTINATION_MATCH", "OUTPUT"):
            os.environ.pop("LUNA_" + name, None)

    def test_a_successful_copy_opens_a_window_with_the_source_info(self):
        cmd = script.luna_command(payload())
        self.assertEqual(cmd[0], script.LUNA_APP)
        self.assertEqual(cmd[1:3], ["report", "/Volumes/RAID 01/Project A/Media/20260521/A001-01"])
        self.assertEqual(
            cmd[3:],
            ["--field", "Date=2026-05-21", "--field", "Shoot Day=01", "--field", "Source Name=A001"],
        )

    def test_the_preset_name_is_not_the_production(self):
        self.assertNotIn("--production", script.luna_command(payload()))

    def test_only_success_gets_a_report(self):
        for state in ("Failed", "Warnings", "Canceled", "Stopped"):
            self.assertIsNone(script.luna_command(payload(FileCopyCompleted_state=state)))

    def test_every_destination_gets_its_own_report(self):
        a = script.luna_command(payload(FileCopyCompleted_destinationPath="/Volumes/T1/A001"))
        b = script.luna_command(payload(FileCopyCompleted_destinationPath="/Volumes/T2/A001"))
        self.assertEqual(a[2], "/Volumes/T1/A001")
        self.assertEqual(b[2], "/Volumes/T2/A001")

    def test_headless_uses_luna_cli_with_json(self):
        os.environ["LUNA_MODE"] = "headless"
        cmd = script.luna_command(payload())
        self.assertEqual(cmd[0], script.LUNA_CLI)
        self.assertEqual(cmd[-1], "--json")

    def test_source_info_may_be_missing_or_broken(self):
        for info in (None, "", "not json", "[1, 2]"):
            cmd = script.luna_command(payload(FileCopyCompleted_sourceInfo=info))
            self.assertNotIn("--field", cmd)

    def test_source_info_values_are_text(self):
        info = {"Shoot Day": 1, "Counter": None}
        cmd = script.luna_command(payload(FileCopyCompleted_sourceInfo=json.dumps(info)))
        self.assertEqual(cmd[3:], ["--field", "Shoot Day=1"])


if __name__ == "__main__":
    unittest.main()
