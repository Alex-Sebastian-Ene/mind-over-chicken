"""Tests for scripts/playtest_summary.py (#94)."""
import contextlib
import csv
import importlib.util
import io
import os
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
spec = importlib.util.spec_from_file_location("playtest_summary", os.path.join(ROOT, "scripts", "playtest_summary.py"))
ps = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ps)

OBS_HEADER = ["participant", "session", "build_sha", "input", "type", "task", "outcome", "seconds", "severity", "issue_code", "note"]


def obs(p, typ, task="", outcome="", seconds="", severity="", code="", note="", session="1"):
    return dict(zip(OBS_HEADER, [p, session, "abc", "emulated", typ, task, outcome, seconds, severity, code, note]))


def sus_row(p, answers, session="1", **extra):
    row = {"participant": p, "session": session}
    row.update({f"sus{i + 1}": str(a) for i, a in enumerate(answers)})
    row.update({k: "50" for k in ps.TLX})
    row.update(extra)
    return row


class SusTest(unittest.TestCase):
    def test_best_and_neutral_and_worst(self):
        self.assertEqual(ps.sus_score(sus_row("P1", [5, 1] * 5)), 100.0)
        self.assertEqual(ps.sus_score(sus_row("P1", [3] * 10)), 50.0)
        self.assertEqual(ps.sus_score(sus_row("P1", [1, 5] * 5)), 0.0)

    def test_incomplete_is_none(self):
        row = sus_row("P1", [3] * 10)
        row["sus4"] = ""
        self.assertIsNone(ps.sus_score(row))


class SummaryTest(unittest.TestCase):
    def setUp(self):
        self.obs = [
            obs("P00", "setup", seconds="999"),  # template example: ignored
            obs("P01", "setup", seconds="90"), obs("P02", "setup", seconds="120"), obs("P03", "setup", seconds="60"),
            obs("P01", "first_action", "T1", seconds="40"), obs("P02", "first_action", "T1", seconds="80"),
            obs("P01", "task", "T1", "S", "100"), obs("P02", "task", "T1", "H", "200"), obs("P03", "task", "T1", "F", "300"),
            obs("P01", "task", "T5", "S"), obs("P02", "task", "T5", "F"),
            # fire-target-unclear: severity 3, 3 of 3 people -> priority 3.0
            obs("P01", "issue", "T1", severity="3", code="fire-target-unclear", note="where do I aim?"),
            obs("P02", "issue", "T1", severity="3", code="fire-target-unclear"),
            obs("P03", "issue", "T1", severity="2", code="fire-target-unclear"),
            # blocker for one person: 4 * 1/3 = 1.33
            obs("P03", "issue", "T3", severity="4", code="fail-loop"),
            # cosmetic for two: 1 * 2/3
            obs("P01", "issue", "T2", severity="1", code="label-overlap"), obs("P02", "issue", "T2", severity="1", code="label-overlap"),
            obs("P01", "confusion", "T2"), obs("P01", "confusion", "T3"),
            # session 2 for P01
            obs("P01", "first_action", "T1", seconds="10", session="2"),
        ]
        self.survey = [
            sus_row("P01", [5, 1] * 5, replay_intent="6"),
            sus_row("P02", [3] * 10, replay_intent="4"),
            sus_row("P01", [5, 1] * 5, session="2", replay_intent="7"),
        ]

    def test_counts_and_example_rows_ignored(self):
        s = ps.summarise(self.obs, self.survey)
        self.assertEqual(s["participants"], 3)
        self.assertEqual(s["setup_seconds"]["median"], 90)

    def test_task_rates(self):
        t1 = ps.summarise(self.obs)["tasks"]["T1"]
        self.assertAlmostEqual(t1["unaided"], 1 / 3)
        self.assertAlmostEqual(t1["any_success"], 2 / 3)
        self.assertAlmostEqual(t1["fail"], 1 / 3)

    def test_issues_ranked_by_severity_times_frequency(self):
        s = ps.summarise(self.obs)
        self.assertEqual(s["top3"], ["fire-target-unclear", "fail-loop", "label-overlap"])
        top = s["issues"][0]
        self.assertEqual(top["severity"], 3)
        self.assertEqual(top["participants"], 3)
        self.assertAlmostEqual(top["priority"], 3.0)
        self.assertEqual(top["example"], "where do I aim?")

    def test_survey_and_retention(self):
        s = ps.summarise(self.obs, self.survey)
        self.assertEqual(s["sus"]["median"], 75.0)  # 100 and 50
        self.assertEqual(s["nasa_tlx_raw"]["median"], 50.0)
        r = s["retention"]
        self.assertEqual(r["returned"], 1)
        self.assertEqual(r["first_action_change_seconds"]["median"], -30.0)
        self.assertEqual(r["replay_intent_change"]["median"], 1.0)

    def test_templates_parse_and_markdown_renders(self):
        root = os.path.join(ROOT, "docs", "playtest")
        o = ps.read(os.path.join(root, "observation_sheet.csv"))
        v = ps.read(os.path.join(root, "survey_responses.csv"))
        self.assertEqual(o, [])  # only P00 examples in the templates
        self.assertEqual(v, [])
        md = ps.markdown(ps.summarise(self.obs, self.survey))
        self.assertIn("fire-target-unclear", md)
        self.assertIn("Retention", md)

    def test_cli_runs_on_files(self):
        with tempfile.TemporaryDirectory() as d:
            path = os.path.join(d, "obs.csv")
            with open(path, "w", newline="", encoding="utf-8") as f:
                w = csv.DictWriter(f, fieldnames=OBS_HEADER)
                w.writeheader()
                w.writerows(self.obs)
            out = io.StringIO()
            with contextlib.redirect_stdout(out):
                self.assertEqual(ps.main([path]), 0)
            self.assertIn("fire-target-unclear", out.getvalue())


if __name__ == "__main__":
    unittest.main()
