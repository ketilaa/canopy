#!/usr/bin/env python3
"""Scores Phase 2 of the structured plan schema
(docs/design/structured-plan-schema-introduction-plan.md), carrying forward A1's (2026-10-20) lesson
that one Implementer run per plan cannot separate a difference of 3 of 10: every arm here runs
`run_step --runs N` (default 3) on each plan, not 1.

Three arms, all on the `tickets` fixture (the only one authored so far):
  a2 (sanity ceiling, not a real arm): the fixed golden fixture, rendered with no model call, run
      through the Implementer --implementer-runs times on the one plan.
  a3 (control): a2 with `decisions` stripped, same --implementer-runs.
  a1 (the real arm): the model drafts each unit's examples/throws for --plans distinct plans
      (run_structured), each then run through the Implementer --implementer-runs times.

usage: score_structured.py --url http://HOST:8080 --out DIR [--plans 10] [--implementer-runs 3]
       [--model ID] [--arms a2,a3,a1] [--prompt lean]
The binaries are the debug examples of the workspace (cargo build -p canopy-planner --examples first).
"""
import argparse
import csv
import glob
import json
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BIN = os.path.join(ROOT, "target", "debug", "examples")
FIXTURE = os.path.join(ROOT, "canopy-implementer", "tasks", "tickets", "plan.structured.yaml")
FEATURE = os.path.join(ROOT, "canopy-implementer", "tasks", "tickets", "feature.feature")


def run(cmd, log):
    with open(log, "w") as f:
        subprocess.run(cmd, stdout=f, stderr=subprocess.STDOUT, cwd=ROOT, check=False)


def score_plan(plan_path, out_dir, implementer_runs, url, model, prompt):
    """Runs `run_step --runs N` once on one plan.yaml and reads its own final tally line
    ("<scored>/<N> runs scored: ..."), rather than the first "hidden acceptance" line — the latter
    would only capture run 1 of N when N > 1."""
    os.makedirs(out_dir, exist_ok=True)
    log = os.path.join(out_dir, "impl.log")
    run(
        [
            os.path.join(BIN, "run_step"),
            "--task", "story:tickets",
            "--plan", plan_path,
            "--runs", str(implementer_runs),
            "--prompt", prompt,
            "--url", url,
            "--model", model,
            "--out", out_dir,
        ],
        log,
    )
    text = open(log).read()
    m = re.search(r"(\d+)/(\d+) runs scored", text)
    scored, total = (int(m.group(1)), int(m.group(2))) if m else (0, implementer_runs)
    return scored, total


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--url", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--plans", type=int, default=10, help="distinct A1 plans to draft")
    ap.add_argument("--implementer-runs", type=int, default=3, help="Implementer runs per plan, every arm")
    ap.add_argument("--model", default="Qwen/Qwen3-14B-GGUF:Q4_K_M")
    ap.add_argument("--arms", default="a2,a3,a1")
    ap.add_argument("--prompt", default="lean")
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    arms = set(a.arms.split(","))
    results = {}

    if "a2" in arms:
        a2_dir = os.path.join(a.out, "a2")
        os.makedirs(a2_dir, exist_ok=True)
        run(
            [os.path.join(BIN, "render_fixed"), "--in", FIXTURE, "--feature", FEATURE, "--out", a2_dir],
            os.path.join(a2_dir, "render.log"),
        )
        scored, total = score_plan(
            os.path.join(a2_dir, "plan.yaml"), os.path.join(a2_dir, "impl"), a.implementer_runs, a.url, a.model, a.prompt
        )
        results["a2"] = {"plans": 1, "scored": scored, "total": total}
        print("a2:", results["a2"], flush=True)

    if "a3" in arms:
        a3_dir = os.path.join(a.out, "a3")
        os.makedirs(a3_dir, exist_ok=True)
        run(
            [os.path.join(BIN, "render_fixed"), "--in", FIXTURE, "--feature", FEATURE, "--out", a3_dir, "--strip-decisions"],
            os.path.join(a3_dir, "render.log"),
        )
        scored, total = score_plan(
            os.path.join(a3_dir, "plan.yaml"), os.path.join(a3_dir, "impl"), a.implementer_runs, a.url, a.model, a.prompt
        )
        results["a3"] = {"plans": 1, "scored": scored, "total": total}
        print("a3:", results["a3"], flush=True)

    if "a1" in arms:
        a1_dir = os.path.join(a.out, "a1")
        plan_dir = os.path.join(a1_dir, "plans")
        os.makedirs(plan_dir, exist_ok=True)
        run(
            [os.path.join(BIN, "run_structured"), "--task", "tickets", "--runs", str(a.plans), "--url", a.url, "--model", a.model, "--out", plan_dir],
            os.path.join(a1_dir, "draft.log"),
        )
        batches = sorted(glob.glob(os.path.join(plan_dir, "2026*")))
        if not batches:
            sys.exit("run_structured produced no batch")
        batch = batches[-1]
        rows = []
        total_scored = total_runs = 0
        for n in range(1, a.plans + 1):
            rd = os.path.join(batch, f"run-{n:02d}")
            plan = os.path.join(rd, "plan.yaml")
            row = {"run": n, "drafted": os.path.exists(plan), "scored": 0, "total": a.implementer_runs}
            if row["drafted"]:
                scored, total = score_plan(plan, os.path.join(a1_dir, f"impl-{n:02d}"), a.implementer_runs, a.url, a.model, a.prompt)
                row["scored"], row["total"] = scored, total
                total_scored += scored
                total_runs += total
            rows.append(row)
            print(row, flush=True)
        with open(os.path.join(a1_dir, "results.csv"), "w", newline="") as f:
            w = csv.DictWriter(f, fieldnames=["run", "drafted", "scored", "total"])
            w.writeheader()
            w.writerows(rows)
        results["a1"] = {
            "plans": a.plans,
            "drafted": sum(1 for r in rows if r["drafted"]),
            "scored": total_scored,
            "total": total_runs,
        }
        print("a1:", results["a1"], flush=True)

    with open(os.path.join(a.out, "summary.json"), "w") as f:
        json.dump(results, f, indent=2)
    print()
    for arm, r in results.items():
        rate = r["scored"] / r["total"] if r["total"] else 0.0
        print(f"{arm}: {r['scored']}/{r['total']} Implementer runs scored (mean {rate:.2f})")


if __name__ == "__main__":
    main()
