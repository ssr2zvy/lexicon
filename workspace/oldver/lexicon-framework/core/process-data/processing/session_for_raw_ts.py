"""session_for_raw_ts.py — ONE function, ONE output (helper convention for
processing scripts): given a data/raw/<ts> timestamp, print/return the id of
the session that recorded it, or nothing/None.

The request folders deliberately carry no session pointer — the archived
session_status.json files under <phase_dir>/sessions/<id>/ are the only
place the correlation exists, so this scans them (live status too, for a
session still running). Processing code stores raw_ts in the DB and derives
the raw session on demand through this helper; the raw session id is never
stored (it would be redundant).

Usage (from a source root, i.e. sources/<name>/):
    python3 .../common/process-data/processing/session_for_raw_ts.py <ts> [phase-dir]
        phase-dir defaults to get-raw-data
    prints the session id (exit 0) or nothing (exit 1)
As a library:
    from session_for_raw_ts import session_for_raw_ts
"""

from __future__ import annotations

import json
import os
import sys
from typing import Optional


def session_for_raw_ts(ts: str, phase_dir: str = "get-raw-data") -> Optional[str]:
    def _records(d: dict) -> list:
        for key in ("network-requests", "work-log"):
            if key in d:
                return d[key] or []
        return []

    base = os.path.join(phase_dir, "sessions")
    candidates = []
    if os.path.isdir(base):
        for sid in os.listdir(base):
            p = os.path.join(base, sid, "session_status.json")
            if os.path.isfile(p):
                candidates.append(p)
    live = os.path.join(phase_dir, "session_status.json")
    if os.path.isfile(live):
        candidates.append(live)
    for path in candidates:
        try:
            with open(path, encoding="utf-8") as f:
                d = json.load(f)
        except Exception:
            continue
        if any(r.get("ts") == ts for r in _records(d)):
            return d.get("session_id")
    return None


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("usage: session_for_raw_ts.py <ts> [phase-dir]", file=sys.stderr)
        sys.exit(2)
    sid = session_for_raw_ts(sys.argv[1],
                             sys.argv[2] if len(sys.argv) > 2 else "get-raw-data")
    if sid:
        print(sid)
        sys.exit(0)
    sys.exit(1)
