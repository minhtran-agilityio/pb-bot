#!/usr/bin/env python3
"""Shape Board spike: drive pd-shape / pd-tickets through `claude -p` with the p2g-questions protocol.

Every run works in a throwaway clone of pb-bot under --work-dir, never in a real checkout.
Transcripts, clones and specs/ stay under --work-dir; nothing here is meant to be committed.

  python3 -I spike.py shape   --repo <pb-bot checkout> --prototype <file.html> --work-dir <dir> --runs 3
  python3 -I spike.py tickets --clone <clone from a shape run> --brief specs/<slug>/brief.md --work-dir <dir>

An auto-answer stands in for the owner: the first option of each question, except "No" on an ADR
question (the ADR step needs git, which the spike denies). `tickets` answers "No milestone" and
stops at the approval question, or at any question it cannot place: it never sends "go".

GitHub safety rests on `--permission-mode dontAsk` plus the allowlist: in that mode only
`--allowedTools` (and commands Claude Code itself treats as read-only) run, and a skill's own
`allowed-tools` pre-allows nothing (checked with a probe skill). No GitHub write is on any
allowlist here. The deny lists are a second line for the writes the skills pre-allow.
"""
import argparse
import glob
import json
import os
import re
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
TURN_TIMEOUT_S = 900

# --permission-mode is pinned: a user or project defaultMode (for example "auto") must not decide.
COMMON = ["--output-format", "stream-json", "--verbose", "--strict-mcp-config",
          "--permission-mode", "dontAsk", "--permission-prompts", "none"]

SHAPE_ALLOWED = ["Read", "Glob", "Grep", "Write(./specs/**)", "Edit(./specs/**)"]
SHAPE_DENIED = ["AskUserQuestion", "Bash", "Agent", "WebFetch", "WebSearch", "NotebookEdit"]

TICKETS_ALLOWED = ["Read", "Glob", "Grep",
                   "Bash(bash *publish-check.sh *)", "Bash(gh repo view *)",
                   "Bash(gh api user --jq .login)", "Bash(gh label list *)", "Bash(gh issue list *)",
                   "Bash(gh api repos/{owner}/{repo}/milestones -X GET -f state=all --paginate)"]
# Second line only (dontAsk + the allowlist above already deny these). It names every write that
# pd-tickets pre-allows, every gh write subcommand family, and the gh api write spellings.
# `gh api` cannot be denied as a whole: deny beats allow, and the exact milestone GET above is needed.
TICKETS_DENIED = ["AskUserQuestion", "Write", "Edit", "Agent", "WebFetch", "WebSearch", "NotebookEdit",
                  "Bash(gh issue create*)", "Bash(gh issue edit*)", "Bash(gh issue comment*)",
                  "Bash(gh issue close*)", "Bash(gh issue reopen*)", "Bash(gh issue delete*)",
                  "Bash(gh issue transfer*)", "Bash(gh issue lock*)", "Bash(gh issue unlock*)",
                  "Bash(gh issue pin*)", "Bash(gh issue unpin*)", "Bash(gh issue develop*)",
                  "Bash(gh label create*)", "Bash(gh label edit*)", "Bash(gh label delete*)",
                  "Bash(gh label clone*)",
                  "Bash(gh api -X*)", "Bash(gh api --method*)", "Bash(gh api * -X POST*)",
                  "Bash(gh api * -X PATCH*)", "Bash(gh api * -X PUT*)", "Bash(gh api * -X DELETE*)",
                  "Bash(gh api * --method*)", "Bash(gh api graphql*)", "Bash(gh api * --input*)",
                  "Bash(gh api * -F*)", "Bash(gh api * --field*)", "Bash(gh api * --raw-field*)",
                  "Bash(gh api * -f title*)", "Bash(gh api * -f name*)", "Bash(gh api * -f body*)",
                  "Bash(gh api * -f description*)", "Bash(gh api * -f state=open*)",
                  "Bash(gh api * -f state=closed*)", "Bash(gh api * -f due_on*)",
                  "Bash(gh pr *)", "Bash(gh repo create*)", "Bash(gh repo edit*)", "Bash(gh repo delete*)",
                  "Bash(gh repo fork*)", "Bash(gh repo rename*)", "Bash(gh repo archive*)",
                  "Bash(gh release *)", "Bash(gh workflow *)", "Bash(gh run *)", "Bash(gh secret *)",
                  "Bash(gh variable *)", "Bash(gh project *)", "Bash(gh gist *)", "Bash(gh auth *)",
                  "Bash(gh extension *)", "Bash(gh alias *)", "Bash(gh config *)", "Bash(gh cache *)",
                  "Bash(git push*)", "Bash(git commit*)"]

# A GitHub write in a Bash command: used to fail the tickets check if one is even attempted.
GH_WRITE_RE = re.compile(
    r"\bgh\s+(issue\s+(create|edit|comment|close|reopen|delete|transfer|lock|unlock|pin|unpin|develop)"
    r"|label\s+(create|edit|delete|clone)|pr\s|repo\s+(create|edit|delete|fork|rename|archive)"
    r"|release\s|workflow\s|run\s|secret\s|variable\s|project\s|gist\s)"
    r"|\bgh\s+api\b.*(\s-X\s*(POST|PATCH|PUT|DELETE)|--method|\sgraphql\b|--input|\s-F\s|--field|--raw-field"
    r"|\s-f\s+(?!state=all\b))"
    r"|\bgit\s+(push|commit)\b")


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
        try:
            proc = subprocess.run(cmd, cwd=cwd, env=env, stdin=subprocess.DEVNULL, stdout=out,
                                  stderr=subprocess.PIPE, text=True, timeout=TURN_TIMEOUT_S)
            code, err = proc.returncode, proc.stderr
        except subprocess.TimeoutExpired:
            code, err = -1, f"timeout after {TURN_TIMEOUT_S} s"
    turn = {"exit_code": code, "stderr": err[-2000:], "seconds": round(time.time() - t0, 1),
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


# An option that would start a write if chosen: "Create 9 issues", "Publish 9 tickets", "Go", ...
GO_LABEL_RE = re.compile(r"\b(create|publish|open|file|post|submit|approve|go|proceed|yes)\b"
                         r"|\d+\s+(issues?|tickets?|milestones?|labels?)\b", re.I)
# The pd-tickets approval: its "go" option names the exact count, e.g. "Create 9 issues".
APPROVAL_LABEL_RE = re.compile(r"^\s*(create|publish|open|file)\s+\d+\s+(new\s+)?(issues?|tickets?)\b", re.I)


def answer_for(block, tickets_mode=False):
    """Pick an answer per question. Returns (answers dict, stop_reason or None).

    In tickets mode the driver must never choose an option that starts a write. It answers only a
    milestone question ("No milestone") and questions whose options are all free of go-words.
    The approval question stops the run with "approval"; any other option that looks like a go
    stops it with "unplaced", which fails the check instead of passing it.
    """
    answers = {}
    for q in block["questions"]:
        labels = [o["label"] for o in q["options"]]
        text = q["question"]
        low = (text + " " + " ".join(labels)).lower()
        if tickets_mode:
            if "milestone" in low and any("no milestone" in lb.lower() for lb in labels):
                answers[text] = next(lb for lb in labels if "no milestone" in lb.lower())
                continue
            if any(APPROVAL_LABEL_RE.search(lb) for lb in labels):
                return answers, "approval"
            if any(GO_LABEL_RE.search(lb) or o.get("preview") for lb, o in zip(labels, q["options"])):
                return answers, "unplaced"
            answers[text] = labels[0]
            continue
        pick = labels[0]
        if re.search(r"\badrs?\b", low):
            pick = next((lb for lb in labels if lb.lower().startswith("no")), pick)
        answers[text] = pick
    return answers, None


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
        "gh_writes": [t["input"].get("command") for t in turn["tool_uses"]
                      if t["name"] == "Bash" and isinstance(t["input"], dict)
                      and GH_WRITE_RE.search(t["input"].get("command") or "")],
        "seconds": turn["seconds"],
        # total_cost_usd is cumulative over --resume: the session cost is the last turn's value.
        "cumulative_cost_usd": (turn["result"] or {}).get("total_cost_usd"),
    }, a


def converse(cwd, run_dir, first_message, base_flags, tickets_mode=False):
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
        answers, stop = answer_for(valid[-1], tickets_mode)
        s["answers"] = answers
        if stop:
            s["stop_reason"] = stop
            s["stopped_at_approval"] = stop == "approval"
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
        specs_dir = os.path.realpath(os.path.join(clone, "specs")) + os.sep
        run = {
            "run": i, "session_id": sid, "turns": len(turns), "question_turns": len(question_turns),
            "invalid_blocks": sum(len(t["invalid_blocks"]) for t in turns),
            "multi_block_turns": sum(1 for t in turns if t["blocks"] > 1),
            # Rule 5: the block ends the turn, so it must be in the turn's last message.
            "block_not_last": [t["turn"] for t in question_turns if not t["block_in_final_message"]],
            # A question in prose in a turn with no block: the protocol failed (it would stall the app).
            "prose_only_questions": [ln for t in turns if not t["blocks"] for ln in t["leak_lines"]],
            # A "?" line next to a valid block (for example a lead-in): reported, not a failure.
            "echo_lines": [ln for t in question_turns for ln in t["leak_lines"]],
            "ask_user_attempts": sum(t["ask_user_attempts"] for t in turns),
            "denials": [d for t in turns for d in t["denials"]],
            "errors": [t["error"] for t in turns if t["error"]],
            "writes": [w for t in turns for w in t["writes"]],
            "writes_outside_specs": [w for t in turns for w in t["writes"]
                                     if not os.path.realpath(os.path.join(clone, w or "")).startswith(specs_dir)],
            "brief": os.path.relpath(briefs[0], clone) if briefs else None,
            "seconds": secs, "cost_usd": turns[-1]["cumulative_cost_usd"] if turns else None,
            "detail": turns,
        }
        run["pass"] = bool(run["brief"] and run["question_turns"] and not run["invalid_blocks"]
                           and not run["ask_user_attempts"] and not run["errors"]
                           and not run["multi_block_turns"] and not run["block_not_last"]
                           and not run["prose_only_questions"] and not run["writes_outside_specs"])
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
                                tickets_mode=True)
    out = {"session_id": sid, "seconds": secs, "turns": turns,
           "cost_usd": turns[-1]["cumulative_cost_usd"] if turns else None,
           "reached_approval": any(t.get("stopped_at_approval") for t in turns),
           "invalid_blocks": sum(len(t["invalid_blocks"]) for t in turns),
           "ask_user_attempts": sum(t["ask_user_attempts"] for t in turns),
           "errors": [t["error"] for t in turns if t["error"]],
           "gh_write_attempts": [c for t in turns for c in t["gh_writes"]],
           "denials": [d for t in turns for d in t["denials"]]}
    out["pass"] = bool(out["reached_approval"] and not out["invalid_blocks"] and not out["ask_user_attempts"]
                       and not out["errors"] and not out["gh_write_attempts"])
    with open(os.path.join(args.work_dir, "tickets-summary.json"), "w", encoding="utf-8") as f:
        json.dump(out, f, indent=2, ensure_ascii=False)
    print(f"tickets: pass={out['pass']} reached_approval={out['reached_approval']} "
          f"gh_write_attempts={len(out['gh_write_attempts'])} denials={len(out['denials'])} {secs}s")
    return 0 if out["pass"] else 1


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
