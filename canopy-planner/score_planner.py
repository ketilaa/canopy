#!/usr/bin/env python3
"""Scores the Planner by what the Implementer does with its plans.

For one task: runs `run_planner` for N plans, then runs `run_step` once on each plan that came out and reads the
hidden-test verdict. Prints a table and writes results.csv in the output directory.

usage: score_planner.py --task stock --runs 10 --url http://HOST:8080 --out DIR [--api] [--prompt lean]
       [--planner-arg ARG]... [--step-arg ARG]...     (extra flags passed through to run_planner / run_step)
The binaries are the debug examples of the workspace (cargo build --examples first).
"""
import argparse, csv, glob, json, os, re, subprocess, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BIN = os.path.join(ROOT, "target", "debug", "examples")


def run(cmd, log):
    with open(log, "w") as f:
        subprocess.run(cmd, stdout=f, stderr=subprocess.STDOUT, cwd=ROOT, check=False)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--task", required=True)
    ap.add_argument("--runs", type=int, default=10)
    ap.add_argument("--url", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--model", default="Qwen/Qwen3-14B-GGUF:Q4_K_M")
    ap.add_argument("--api", action="store_true")
    ap.add_argument("--prompt", default="lean")
    ap.add_argument("--planner-arg", action="append", default=[])
    ap.add_argument("--step-arg", action="append", default=[])
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    plan_dir = os.path.join(a.out, "plans")
    os.makedirs(plan_dir, exist_ok=True)
    cmd = [os.path.join(BIN, "run_planner"), "--task", a.task, "--runs", str(a.runs), "--url", a.url,
           "--model", a.model, "--out", plan_dir] + (["--api"] if a.api else []) + a.planner_arg
    run(cmd, os.path.join(a.out, "planner.log"))
    batches = sorted(glob.glob(os.path.join(plan_dir, "2026*")))
    if not batches:
        sys.exit("run_planner produced no batch")
    batch = batches[-1]
    rows = []
    for n in range(1, a.runs + 1):
        rd = os.path.join(batch, f"run-{n:02d}")
        plan = os.path.join(rd, "plan.yaml")
        meta = {}
        try:
            meta = json.load(open(os.path.join(rd, "summary.json")))
        except Exception:
            pass
        row = {"run": n, "valid": os.path.exists(plan), "steps_planned": "", "steps_passed": "", "hidden": "not_run"}
        if row["valid"]:
            sd = os.path.join(a.out, f"impl-{n:02d}")
            os.makedirs(sd, exist_ok=True)
            log = os.path.join(a.out, f"impl-{n:02d}.log")
            run([os.path.join(BIN, "run_step"), "--task", f"story:{a.task}" if a.task != "order" else "order-planned",
                 "--plan", plan, "--runs", "1", "--prompt", a.prompt, "--url", a.url, "--model", a.model, "--out", sd] + a.step_arg, log)
            text = open(log).read()
            m = re.search(r"== plan \w+: (PASSED|STOPPED) \((\d+) of (\d+) steps?\)", text)
            if m:
                row["steps_passed"], row["steps_planned"] = m.group(2), m.group(3)
            m = re.search(r"hidden acceptance: (\w+)", text)
            if m:
                row["hidden"] = m.group(1)
        rows.append(row)
        print(row, flush=True)
    with open(os.path.join(a.out, "results.csv"), "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=["run", "valid", "steps_planned", "steps_passed", "hidden"])
        w.writeheader()
        w.writerows(rows)
    valid = sum(r["valid"] for r in rows)
    passed = sum(r["hidden"] == "passed" for r in rows)
    print(f"task {a.task}: {valid}/{len(rows)} valid plans, {passed}/{len(rows)} hidden-test passes; batch {batch}")


if __name__ == "__main__":
    main()
