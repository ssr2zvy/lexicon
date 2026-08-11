"""session_engine.py — PRIVATE library code (common/shared/, the one
sanctioned shared location). Used by EXACTLY TWO files:
common/get-raw-data/get_raw_data.py and common/process-data/process_data.py.
Nothing else may import it — source impls only ever import their phase
façade, whose __all__ is the entire public contract. Nothing here is
source- or phase-specific: the phase modules specialize DataSource with
three class attributes and (where needed) an abandon_extra() hook.

Framework-owned, identical in both phases:
  * CLI universals: --bg (detach; NOT an input variable) and
    --abandon-recent-failed (required to change source-specific input
    variables while the latest session's run_status is "fail").
  * Session kinds new|retry encoded in the session id <utc-ts>-<kind>.
  * Live <phase>/session_status.json:
      { session_id, kind, run_status: running|success|fail,
        scope: {...input variables...},
        cursor: { scope, leftover_position },
        <records_key>: [ {ts, type}, ... ],       # timestamp + type ONLY
        started, finished, rc }
  * <phase>/sessions/<id>/ record: session.txt, log (stdout+stderr tee'd
    live), session_status.json archived at exit.
  * data/raw/<ts>/{request,response}/ folders for every network request; no
    session pointer inside — the ts list in the status is the correlation.
  * Truthful finalization: SIGTERM/SIGINT raise SystemExit(143/130) so the
    finally block always records how the session really ended.
  * Refuse/abandon: previous fail + different scope -> exit 2 unless
    --abandon-recent-failed, which deletes the failed session's recorded
    data/raw/<ts> folders, its sessions/<id>/ record, the live status, and
    whatever the phase's abandon_extra() adds.
  * Retry drive: leftover_position discernible -> source.resume(pos); else
    source.fallback_position(previous records) -> resume(pos) or run().
"""

from __future__ import annotations

import abc
import fcntl
import json
import os
import shutil
import signal
import subprocess
import sys
import time
import traceback
from typing import Any, Optional, Sequence


def utcstamp() -> str:
    return time.strftime("%Y%m%dT%H%M%SZ", time.gmtime())


def utcnow() -> str:
    return time.strftime("%Y-%m-%d %H:%M:%S", time.gmtime())


class _Status:
    def __init__(self, phase_dir: str):
        self.path = os.path.join(phase_dir, "session_status.json")

    def load(self) -> dict:
        try:
            with open(self.path, encoding="utf-8") as f:
                return json.load(f)
        except Exception:
            return {}

    def save(self, d: dict) -> None:
        with open(self.path, "w", encoding="utf-8") as f:
            json.dump(d, f, ensure_ascii=False)


class Session:
    """Framework-owned handle passed to the source. Sources never touch
    session_status.json or sessions/<id>/ directly — only through this."""

    def __init__(self, session_id: str, kind: str, scope: dict,
                 status: _Status, records_key: str):
        self.session_id = session_id
        self.kind = kind            # "new" | "retry"
        self.scope = scope
        self._status = status
        self._records_key = records_key

    def request_dir(self, type: str) -> str:
        """Create data/raw/<ts>/{request,response} for a network request,
        append {ts, type} to the session's record list, return ts."""
        base = utcstamp()
        ts, n = base, 0
        while os.path.exists(os.path.join("data", "raw", ts)):
            n += 1
            ts = f"{base}-{n:03d}"
        os.makedirs(os.path.join("data", "raw", ts, "request"))
        os.makedirs(os.path.join("data", "raw", ts, "response"))
        self.record(ts, type)
        return ts

    def record(self, ts: str, type: str) -> None:
        """Append {ts, type} to the record list without creating a folder —
        for retries re-entering an existing data/raw/<ts> (e.g. curl -C on a
        partial) or, in the process phase, non-network work steps (which then
        use a fresh utcstamp() with no folder)."""
        d = self._status.load()
        d.setdefault(self._records_key, []).append({"ts": ts, "type": type})
        self._status.save(d)

    def set_info(self, info: dict) -> None:
        """Merge non-scope inputs the implementation wants each session to
        self-document (e.g. a hardcoded dependency path + SQL query, or the
        sha256 of a consumed DB) into the status's "inputs" field. These are
        NOT scope variables: they never participate in the retry/refuse
        comparison — they are recorded for provenance only, and land in the
        archived session_status.json like everything else."""
        d = self._status.load()
        d.setdefault("inputs", {}).update(info)
        self._status.save(d)

    def set_position(self, position: Any) -> None:
        """Persist cursor.leftover_position (source-defined shape; None
        clears it). Call after every unit of progress."""
        d = self._status.load()
        d.setdefault("cursor", {})["leftover_position"] = position
        self._status.save(d)

    def log(self, msg: str) -> None:
        print(msg, flush=True)   # stdout is tee'd into the session log


class DataSource(abc.ABC):
    """Phase modules subclass this into RawDataSource / ProcessDataSource;
    sources subclass those. The framework instantiates the source, resolves
    the session (new/retry/refuse/abandon), then drives run()/resume();
    return normally for success, raise / SystemExit(nonzero) for fail."""

    name: str = ""
    usage: str = "[--bg] [--abandon-recent-failed]"
    phase_dir: str = ""       # "get-raw-data" | "process-data"
    records_key: str = ""     # "network-requests" | "work-log"
    cli: str = ""             # outward-facing dispatcher name, for messages

    def prepare(self, argv: Sequence[str]) -> Optional[int]:
        """Pre-session hook, called BEFORE parse_scope and before any --bg
        detach (so e.g. credentials it puts into os.environ are inherited by
        the background child). Return None to proceed normally, or an exit
        code to short-circuit main() entirely (for session-less side modes
        like wikimedia's --fetch-audio). argv is the CLI args with the
        universal flags already stripped."""
        return None

    @abc.abstractmethod
    def parse_scope(self, argv: Sequence[str]) -> dict:
        """Parse source-specific input variables into the scope dict. Raise
        ValueError on unknown/invalid arguments. Sources without input
        variables return {} and reject any argv."""

    @abc.abstractmethod
    def run(self, session: Session) -> None:
        """Fresh work from the start of the scope."""

    @abc.abstractmethod
    def resume(self, session: Session, position: Any) -> None:
        """The source's unique continue protocol, given the discernible
        cursor.leftover_position inherited from the failed session."""

    @abc.abstractmethod
    def fallback_position(self, session: Session,
                          previous_records: Sequence[dict]) -> Optional[Any]:
        """Reconstruct a resume position when leftover_position was not
        discernible, by inspecting the failed session's recorded work
        ({ts, type} -> data/raw/<ts>/ where a folder exists). Return None to
        run() from scratch."""

    def abandon_extra(self, failed_status: dict) -> None:
        """Phase- or source-specific extra cleanup during
        --abandon-recent-failed (after the recorded data/raw/<ts> folders,
        the session record and the live status are already gone)."""


class Throttle:
    """Shared cross-process flock token bucket (same protocol as the old
    tree, so concurrent tools stay coordinated)."""

    def __init__(self, path: str, interval: float):
        self.path = path
        self.interval = interval

    def take(self) -> str:
        with open(self.path, "a+") as fh:
            while True:
                fcntl.flock(fh, fcntl.LOCK_EX)
                fh.seek(0)
                parts = fh.read().split()
                next_at = float(parts[0]) if parts else 0.0
                seq = int(parts[1]) if len(parts) > 1 else 0
                now = time.time()
                if now >= next_at:
                    seq += 1
                    fh.seek(0); fh.truncate()
                    fh.write(f"{now + self.interval} {seq}")
                    fh.flush()
                    fcntl.flock(fh, fcntl.LOCK_UN)
                    return f"{utcstamp()}-{seq:06d}"
                wait = next_at - now
                fcntl.flock(fh, fcntl.LOCK_UN)
                time.sleep(min(wait, 5))

    def penalize(self, seconds: float) -> None:
        with open(self.path, "a+") as fh:
            fcntl.flock(fh, fcntl.LOCK_EX)
            fh.seek(0)
            parts = fh.read().split()
            cur = float(parts[0]) if parts else 0.0
            seq = int(parts[1]) if len(parts) > 1 else 0
            fh.seek(0); fh.truncate()
            fh.write(f"{max(cur, time.time() + seconds)} {seq}")
            fcntl.flock(fh, fcntl.LOCK_UN)


class _Tee:
    def __init__(self, *streams):
        self._streams = streams

    def write(self, data):
        for s in self._streams:
            s.write(data)
        self.flush()

    def flush(self):
        for s in self._streams:
            try:
                s.flush()
            except Exception:
                pass


def main(source: DataSource, argv: Sequence[str]) -> int:
    status = _Status(source.phase_dir)
    bg = "--bg" in argv
    abandon = "--abandon-recent-failed" in argv
    rest = [a for a in argv if a not in ("--bg", "--abandon-recent-failed")]
    early = source.prepare(rest)
    if early is not None:
        return early
    try:
        scope = source.parse_scope(rest)
    except ValueError as e:
        print(f"error: {e}", file=sys.stderr)
        print(f"usage: {source.cli} {source.name} {source.usage}",
              file=sys.stderr)
        return 2

    # --- session resolution (new / retry / refuse / abandon) -----------------
    st = status.load()
    kind, inherited, prev = "new", None, None
    if st.get("run_status") == "fail":
        if abandon:
            # session_status.json is the source of truth for the failed
            # session's recorded work: delete each recorded data/raw/<ts>
            # that exists, the session record, the live status, then any
            # phase/source extra cleanup.
            for req in st.get(source.records_key, []):
                ts = req.get("ts", "")
                d = os.path.join("data", "raw", ts)
                if ts and os.path.isdir(d):
                    shutil.rmtree(d)
            sid = st.get("session_id", "")
            sd = os.path.join(source.phase_dir, "sessions", sid)
            if sid and os.path.isdir(sd):
                shutil.rmtree(sd)
            os.remove(status.path)
            source.abandon_extra(st)
        elif scope != st.get("scope", {}):
            print("error: the most recent session FAILED and used different "
                  "input variables:", file=sys.stderr)
            print(f"       previous scope: "
                  f"{json.dumps(st.get('scope', {}), ensure_ascii=False)}",
                  file=sys.stderr)
            print("       Re-run with the same variables to retry it, or add "
                  "--abandon-recent-failed", file=sys.stderr)
            print("       to delete that session (its recorded data/raw "
                  "folders, record, log and status) and start fresh.",
                  file=sys.stderr)
            return 2
        else:
            kind, inherited, prev = "retry", dict(st.get("cursor") or {}), st

    # --- background detach (after refuse/abandon are settled) ----------------
    if bg:
        # Re-invoke exactly what was invoked (the phase run.py with the
        # source name), minus the universal flags already handled here.
        argv_wo_flags = [a for a in sys.argv[1:]
                         if a not in ("--bg", "--abandon-recent-failed")]
        with open(os.devnull, "rb") as din, open(os.devnull, "wb") as dout:
            subprocess.Popen([sys.executable, os.path.abspath(sys.argv[0]),
                              *argv_wo_flags],
                             stdin=din, stdout=dout, stderr=dout,
                             start_new_session=True, cwd=os.getcwd())
        print(f"started in background; log: "
              f"{source.phase_dir}/sessions/<session id>/log")
        return 0

    # --- session record + live status ----------------------------------------
    session_id = f"{utcstamp()}-{kind}"
    sdir = os.path.join(source.phase_dir, "sessions", session_id)
    os.makedirs(sdir, exist_ok=True)
    with open(os.path.join(sdir, "session.txt"), "w", encoding="utf-8") as f:
        f.write(f"id: {session_id}\nkind: {kind}\nstarted: {utcnow()}\n")
    cursor = dict(inherited or {})
    cursor.setdefault("leftover_position", None)
    cursor["scope"] = scope
    status.save({"session_id": session_id, "kind": kind,
                 "run_status": "running", "scope": scope, "cursor": cursor,
                 source.records_key: [], "started": utcnow()})

    log_fh = open(os.path.join(sdir, "log"), "a", encoding="utf-8")
    sys.stdout = _Tee(sys.__stdout__, log_fh)
    sys.stderr = _Tee(sys.__stderr__, log_fh)
    # Make signal deaths reach the finally: block so the recorded status never
    # lies about how the session ended.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(143))
    signal.signal(signal.SIGINT, lambda *_: sys.exit(130))

    sess = Session(session_id, kind, scope, status, source.records_key)
    rc = 0
    try:
        print(f"session {session_id} ({kind})")
        if kind == "retry":
            pos = (inherited or {}).get("leftover_position")
            if pos is None:
                pos = source.fallback_position(
                    sess, (prev or {}).get(source.records_key, []))
            if pos is not None:
                source.resume(sess, pos)
            else:
                print("retry: no position discernible even via fallback — "
                      "starting the work from the beginning")
                source.run(sess)
        else:
            source.run(sess)
    except SystemExit as e:
        rc = e.code if isinstance(e.code, int) else 1
    except Exception:
        traceback.print_exc()
        rc = 1
    finally:
        d = status.load()
        d["run_status"] = "success" if rc == 0 else "fail"
        d["finished"] = utcnow()
        d["rc"] = rc
        status.save(d)
        log_fh.flush()
        try:
            shutil.copyfile(status.path,
                            os.path.join(sdir, "session_status.json"))
        except Exception:
            pass
    return rc
