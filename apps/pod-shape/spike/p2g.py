"""p2g-questions protocol helpers: find the blocks in assistant text, validate them, spot leaks.

Stdlib only. The rules mirror p2g-questions.schema.json (same shape as the AskUserQuestion input).
Run `python3 -I p2g.py --selftest` to check the helpers offline.
"""
import json
import re
import sys

TAG = "p2g-questions"


def split_fences(text):
    """Return (blocks, outside): blocks = [(info, body)], outside = text outside every fence."""
    blocks, outside, open_fence = [], [], None
    lines = text.split("\n")
    buf, info = [], ""
    for line in lines:
        m = re.match(r"^[ \t]*(```+|~~~+)[ \t]*([^`]*?)[ \t]*$", line)
        if open_fence is None:
            if m:
                open_fence, info, buf = m.group(1), m.group(2).strip(), []
            else:
                outside.append(line)
        else:
            if m and m.group(1)[0] == open_fence[0] and len(m.group(1)) >= len(open_fence) and not m.group(2):
                blocks.append((info, "\n".join(buf)))
                open_fence = None
            else:
                buf.append(line)
    if open_fence is not None:  # unclosed fence: keep it as a block so it is validated (and fails)
        blocks.append((info + " (unclosed)", "\n".join(buf)))
    return blocks, "\n".join(outside)


def _err(path, msg):
    return f"{path}: {msg}"


def validate(obj):
    """Return a list of errors; empty means valid against p2g-questions.schema.json."""
    errs = []
    if not isinstance(obj, dict):
        return [_err("$", "not an object")]
    extra = set(obj) - {"questions"}
    if extra:
        errs.append(_err("$", f"unexpected keys {sorted(extra)}"))
    qs = obj.get("questions")
    if not isinstance(qs, list) or not 1 <= len(qs) <= 4:
        return errs + [_err("$.questions", "must be an array of 1 to 4 items")]
    for i, q in enumerate(qs):
        p = f"$.questions[{i}]"
        if not isinstance(q, dict):
            errs.append(_err(p, "not an object"))
            continue
        extra = set(q) - {"question", "header", "options", "multiSelect"}
        if extra:
            errs.append(_err(p, f"unexpected keys {sorted(extra)}"))
        for key in ("question", "header"):
            if not isinstance(q.get(key), str) or not q.get(key):
                errs.append(_err(f"{p}.{key}", "must be a non-empty string"))
        if isinstance(q.get("header"), str) and len(q["header"]) > 12:
            errs.append(_err(f"{p}.header", f"longer than 12 characters ({len(q['header'])})"))
        if not isinstance(q.get("multiSelect"), bool):
            errs.append(_err(f"{p}.multiSelect", "must be a boolean"))
        opts = q.get("options")
        if not isinstance(opts, list) or not 2 <= len(opts) <= 4:
            errs.append(_err(f"{p}.options", "must be an array of 2 to 4 items"))
            continue
        for j, o in enumerate(opts):
            op = f"{p}.options[{j}]"
            if not isinstance(o, dict):
                errs.append(_err(op, "not an object"))
                continue
            extra = set(o) - {"label", "description", "preview"}
            if extra:
                errs.append(_err(op, f"unexpected keys {sorted(extra)}"))
            if not isinstance(o.get("label"), str) or not o.get("label"):
                errs.append(_err(f"{op}.label", "must be a non-empty string"))
            if not isinstance(o.get("description"), str):
                errs.append(_err(f"{op}.description", "must be a string"))
            if "preview" in o and not isinstance(o["preview"], str):
                errs.append(_err(f"{op}.preview", "must be a string"))
    return errs


def analyse(text):
    """Check one assistant message. Returns a dict with blocks, errors and leak lines."""
    blocks, outside = split_fences(text)
    found = []
    for info, body in blocks:
        if TAG not in info:
            continue
        entry = {"info": info, "errors": [], "data": None}
        if "(unclosed)" in info:
            entry["errors"].append("unclosed fence")
        try:
            entry["data"] = json.loads(body)
        except ValueError as e:
            entry["errors"].append(f"invalid JSON: {e}")
        else:
            entry["errors"] += validate(entry["data"])
        found.append(entry)
    # A leak: a question to the owner written outside the block. Heuristic: a prose line ending in "?".
    leaks = [ln.strip() for ln in outside.split("\n")
             if ln.strip().rstrip("*_ )").endswith("?") and not ln.strip().startswith(("|", ">"))]
    if TAG in outside:
        leaks.append(f"'{TAG}' named outside a fenced block")
    return {"blocks": found, "leaks": leaks}


def _selftest():
    good = {"questions": [{"question": "Pick one?", "header": "scope", "multiSelect": False,
                           "options": [{"label": "A", "description": "a"}, {"label": "B", "description": "b"}]}]}
    msg = "Intro text.\n\n```p2g-questions\n" + json.dumps(good) + "\n```\n"
    r = analyse(msg)
    assert len(r["blocks"]) == 1 and not r["blocks"][0]["errors"] and not r["leaks"], r
    bad = dict(good, questions=[dict(good["questions"][0], header="far too long a header", options=[])])
    r = analyse("```p2g-questions\n" + json.dumps(bad) + "\n```")
    assert len(r["blocks"][0]["errors"]) == 2, r
    r = analyse("```p2g-questions\n{not json}\n```")
    assert r["blocks"][0]["errors"][0].startswith("invalid JSON"), r
    r = analyse("Does this brief say what you mean?\n")
    assert r["leaks"] == ["Does this brief say what you mean?"] and not r["blocks"], r
    r = analyse("```json\n{\"a\": \"why?\"}\n```\nDone.")
    assert not r["blocks"] and not r["leaks"], r
    r = analyse("```p2g-questions\n" + json.dumps(good))
    assert r["blocks"][0]["errors"] == ["unclosed fence"], r
    print("p2g selftest: ok")


if __name__ == "__main__":
    if sys.argv[1:] == ["--selftest"]:
        _selftest()
    else:
        print("usage: python3 -I p2g.py --selftest", file=sys.stderr)
        sys.exit(2)
