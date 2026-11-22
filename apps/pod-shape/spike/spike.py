#!/usr/bin/env python3
"""Shape Board spike: drive pd-shape / pd-tickets through `claude -p` with the p2g-questions protocol.

Every run works in a throwaway clone of pb-bot under --work-dir, never in a real checkout.
Transcripts, clones and specs/ stay under --work-dir; nothing here is meant to be committed.

  python3 -I spike.py shape   --repo <pb-bot checkout> --prototype <file.html> --work-dir <dir> --runs 3
  python3 -I spike.py tickets --clone <clone from a shape run> --brief specs/<slug>/brief.md --work-dir <dir>

An auto-answer stands in for the owner: the first option of each question, except "No" on an ADR
question (the ADR step needs git, which the spike denies). `tickets` answers "No milestone" and
stops at the approval question: it never sends "go", and the flags deny every GitHub write.
"""
import argparse
import glob
import json
import os
import shutil
import subprocess
import sys
import time
import uuid

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import p2g  # noqa: E402

PROMPT_FILE = os.path.join(HERE, "append-system-prompt.md")
MAX_TURNS = 16

COMMON = ["--output-format", "stream-json", "--verbose", "--strict-mcp-config",
          "--permission-prompts", "none"]

SHAPE_ALLOWED = ["Read", "Glob", "Grep", "Write(./specs/**)", "Edit(./specs/**)"]
SHAPE_DENIED = ["AskUserQuestion", "Bash", "Agent", "WebFetch", "WebSearch", "NotebookEdit"]

TICKETS_ALLOWED = ["Read", "Glob", "Grep",
                   "Bash(bash *publish-check.sh *)", "Bash(gh repo view *)",
                   "Bash(gh api user --jq .login)", "Bash(gh label list *)", "Bash(gh issue list *)",
                   "Bash(gh api repos/{owner}/{repo}/milestones -X GET -f state=all --paginate)"]
TICKETS_DENIED = ["AskUserQuestion", "Write", "Edit", "Agent", "WebFetch", "WebSearch", "NotebookEdit",
                  "Bash(gh issue create*)", "Bash(gh issue edit*)", "Bash(gh issue comment*)",
                  "Bash(gh label create*)", "Bash(gh label edit*)",
                  "Bash(gh api repos/{owner}/{repo}/milestones -f*)", "Bash(gh api * -f title*)",
                  "Bash(gh api * -X POST*)", "Bash(gh api * --method*)", "Bash(gh pr *)",
                  "Bash(git push*)", "Bash(git commit*)"]


def flags(model, effort, allowed, denied):
    with open(PROMPT_FILE, encoding="utf-8") as f:
        prompt = f.read()
    return (COMMON + ["--model", model, "--effort", effort, "--append-system-prompt", prompt,
                      "--allowedTools", *allowed, "--disallowedTools", *denied])


def run_turn(cwd, log_path, session_flag, message, base_flags):
    """One `claude -p` call. Returns a dict with text, tool uses, denials, result and timing."""
    cmd = ["claude", "-p", *session_flag, *base_flags, "--", message]
    env = dict(os.environ, PD_BOT_ROLE="none")
    t0 = time.time()
    with open(log_path, "w", encoding="utf-8") as out:
        proc = subprocess.run(cmd, cwd=cwd, env=env, stdout=out, stderr=subprocess.PIPE, text=True)
    turn = {"exit_code": proc.returncode, "stderr": proc.stderr[-2000:], "seconds": round(time.time() - t0, 1),
            "texts": [], "tool_uses": [], "denials": [], "result": None, "slash_commands": None}
    with open(log_path, encoding="utf-8") as f:
        for line in f:
            try:
                o = json.loads(line)
            except ValueError:
                continue
            if o.get("type") == "system" and o.get("subtype") == "init":
                turn["slash_commands"] = o.get("slash_commands")
            if o.get("type") == "assistant" and not o.get("parent_tool_use_id"):
                for b in o.get("message", {}).get("content", []):
                    if b.get("type") == "text":
                        turn["texts"].append(b["text"])
                    elif b.get("type") == "tool_use":
                        turn["tool_uses"].append({"name": b.get("name"), "input": b.get("input")})
            if o.get("type") == "result":
                turn["result"] = {k: o.get(k) for k in ("subtype", "is_error", "num_turns", "duration_ms",
                                                         "total_cost_usd", "result")}
                turn["denials"] = o.get("permission_denials") or []
    return turn


def answer_for(block, stop_on_approval=False):
    """Pick an answer per question. Returns (answers dict, approval_seen)."""
    answers, approval = {}, False
    for q in block["questions"]:
        labels = [o["label"] for o in q["options"]]
        text = q["question"]
        low = (text + " " + " ".join(labels)).lower()
        if stop_on_approval and ("create" in low and "issue" in low):
            approval = True
        pick = labels[0]
        if "adr" in low:
            pick = next((lb for lb in labels if lb.lower().startswith("no")), pick)
        if "milestone" in low:
            pick = next((lb for lb in labels if "no milestone" in lb.lower()), pick)
        answers[text] = pick
    return answers, approval


def summarise_turn(turn):
    text = "\n\n".join(turn["texts"])
    a = p2g.analyse(text)
    final = (turn["result"] or {}).get("result") or ""
    a_final = p2g.analyse(final)
    return {
        "blocks": len(a["blocks"]),
        "valid_blocks": sum(1 for b in a["blocks"] if not b["errors"]),
        "invalid_blocks": [b["errors"] for b in a["blocks"] if b["errors"]],
        "block_in_final_message": bool(a_final["blocks"]),
        "leak_lines": a["leaks"],
        "ask_user_attempts": sum(1 for t in turn["tool_uses"] if t["name"] == "AskUserQuestion"),
        "tools": [t["name"] for t in turn["tool_uses"]],
        "writes": [t["input"].get("file_path") for t in turn["tool_uses"]
                   if t["name"] in ("Write", "Edit") and isinstance(t["input"], dict)],
        "denials": [{"tool": d.get("tool_name"), "input": d.get("tool_input")} for d in turn["denials"]],
        "error": None if (turn["exit_code"] == 0 and turn["result"] and not turn["result"]["is_error"])
        else {"exit_code": turn["exit_code"], "result": turn["result"], "stderr": turn["stderr"]},
        "seconds": turn["seconds"],
        "cost_usd": (turn["result"] or {}).get("total_cost_usd"),
    }, a


def converse(cwd, run_dir, first_message, base_flags, stop_on_approval=False):
    sid = str(uuid.uuid4())
    turns, message, t0 = [], first_message, time.time()
    for n in range(1, MAX_TURNS + 1):
        session_flag = ["--session-id", sid] if n == 1 else ["--resume", sid]
        turn = run_turn(cwd, os.path.join(run_dir, f"turn-{n:02d}.jsonl"), session_flag, message, base_flags)
        s, a = summarise_turn(turn)
        s["turn"] = n
        if n == 1:
            s["slash_commands_has"] = {c: c in (turn["slash_commands"] or [])
                                       for c in ("pd-bot:pd-shape", "pd-bot:pd-tickets")}
        with open(os.path.join(run_dir, f"turn-{n:02d}.text.md"), "w", encoding="utf-8") as f:
            f.write("\n\n".join(turn["texts"]))
        valid = [b["data"] for b in a["blocks"] if not b["errors"]]
        s["questions"] = [q["question"] for b in valid for q in b["questions"]]
        turns.append(s)
        print(f"  turn {n}: blocks={s['blocks']} valid={s['valid_blocks']} leaks={len(s['leak_lines'])} "
              f"ask={s['ask_user_attempts']} {s['seconds']}s err={'yes' if s['error'] else 'no'}", flush=True)
        if s["error"] or not valid:
            break
        answers, approval = answer_for(valid[-1], stop_on_approval)
        s["answers"] = answers
        if approval:
            s["stopped_at_approval"] = True
            break
        message = json.dumps({"answers": answers}, ensure_ascii=False)
    return sid, turns, round(time.time() - t0, 1)


def make_clone(repo, dest, prototype):
    subprocess.run(["git", "clone", "--quiet", repo, dest], check=True)
    # GitHub reads work (pd-tickets needs gh repo view); every push fails.
    url = subprocess.run(["git", "-C", repo, "remote", "get-url", "origin"], capture_output=True,
                         text=True).stdout.strip()
    if url:
        subprocess.run(["git", "-C", dest, "remote", "set-url", "origin", url], check=True)
    subprocess.run(["git", "-C", dest, "remote", "set-url", "--push", "origin", "file:///dev/null/no-push"],
                   check=True)
    os.makedirs(os.path.join(dest, "prototypes"), exist_ok=True)
    shutil.copy(prototype, os.path.join(dest, "prototypes", os.path.basename(prototype)))
    with open(os.path.join(dest, ".git", "info", "exclude"), "a", encoding="utf-8") as f:
        f.write("\nprototypes/\n")  # keeps git status clean, so the Stop hook sees no code change


def cmd_shape(args):
    results = []
    for i in range(1, args.runs + 1):
        run_dir = os.path.join(args.work_dir, f"shape-run-{i}")
        os.makedirs(run_dir, exist_ok=True)
        clone = os.path.join(run_dir, "clone")
        make_clone(args.repo, clone, args.prototype)
        first = f"/pd-bot:pd-shape prototypes/{os.path.basename(args.prototype)}"
        print(f"run {i}: {first}", flush=True)
        sid, turns, secs = converse(clone, run_dir, first,
                                    flags(args.model, args.effort, SHAPE_ALLOWED, SHAPE_DENIED))
        briefs = sorted(glob.glob(os.path.join(clone, "specs", "*", "brief.md")))
        question_turns = [t for t in turns if t["blocks"]]
        run = {
            "run": i, "session_id": sid, "turns": len(turns), "question_turns": len(question_turns),
            "invalid_blocks": sum(len(t["invalid_blocks"]) for t in turns),
            "multi_block_turns": sum(1 for t in turns if t["blocks"] > 1),
            "leak_lines": [ln for t in turns for ln in t["leak_lines"]],
            "ask_user_attempts": sum(t["ask_user_attempts"] for t in turns),
            "denials": [d for t in turns for d in t["denials"]],
            "errors": [t["error"] for t in turns if t["error"]],
            "writes": [w for t in turns for w in t["writes"]],
            "brief": os.path.relpath(briefs[0], clone) if briefs else None,
            "seconds": secs, "cost_usd": round(sum(t["cost_usd"] or 0 for t in turns), 4),
            "detail": turns,
        }
        run["pass"] = bool(run["brief"] and run["question_turns"] and not run["invalid_blocks"]
                           and not run["ask_user_attempts"] and not run["errors"]
                           and not run["multi_block_turns"])
        print(f"run {i}: pass={run['pass']} turns={run['turns']} brief={run['brief']} {secs}s", flush=True)
        results.append(run)
        with open(os.path.join(args.work_dir, "shape-summary.json"), "w", encoding="utf-8") as f:
            json.dump(results, f, indent=2, ensure_ascii=False)
        if not run["pass"] and not args.keep_going:
            break
    print(f"PASS {sum(r['pass'] for r in results)}/{args.runs}")
    return 0 if len(results) == args.runs and all(r["pass"] for r in results) else 1


def cmd_tickets(args):
    run_dir = os.path.join(args.work_dir, "tickets-check")
    os.makedirs(run_dir, exist_ok=True)
    first = f"/pd-bot:pd-tickets {args.brief}"
    print(f"tickets: {first}", flush=True)
    sid, turns, secs = converse(args.clone, run_dir, first,
                                flags(args.model, args.effort, TICKETS_ALLOWED, TICKETS_DENIED),
                                stop_on_approval=True)
    out = {"session_id": sid, "seconds": secs, "turns": turns,
           "reached_approval": any(t.get("stopped_at_approval") for t in turns),
           "create_attempts": [t for turn in turns for t in turn["denials"]
                               if "create" in json.dumps(t.get("input") or "")]}
    with open(os.path.join(args.work_dir, "tickets-summary.json"), "w", encoding="utf-8") as f:
        json.dump(out, f, indent=2, ensure_ascii=False)
    print(f"tickets: reached_approval={out['reached_approval']} {secs}s")
    return 0 if out["reached_approval"] else 1


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("shape")
    s.add_argument("--repo", required=True)
    s.add_argument("--prototype", required=True)
    s.add_argument("--runs", type=int, default=3)
    s.add_argument("--keep-going", action="store_true", help="run all runs even after a failed one")
    t = sub.add_parser("tickets")
    t.add_argument("--clone", required=True)
    t.add_argument("--brief", required=True)
    for p in (s, t):
        p.add_argument("--work-dir", required=True)
        p.add_argument("--model", default="sonnet")
        p.add_argument("--effort", default="medium")
    args = ap.parse_args()
    args.work_dir = os.path.abspath(args.work_dir)
    os.makedirs(args.work_dir, exist_ok=True)
    return cmd_shape(args) if args.cmd == "shape" else cmd_tickets(args)


if __name__ == "__main__":
    sys.exit(main())
