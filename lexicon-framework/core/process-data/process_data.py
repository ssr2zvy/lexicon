"""process_data.py — the process-data phase FAÇADE. __all__ below is the
ENTIRE public contract between the framework and the source impls
(sources/<name>/process-data/process_data_impl.py): impls import ONLY this
module, and only these names; the engine (common/shared/session_engine.py)
is private and has exactly two importers — this file and its get-raw-data
twin.

Entry flow (single entry point, no exceptions):
  common/process-data/process-data.sh <source> [options...]
    -> exec common/process-data/run.py <source> [options...]
       -> chdir sources/<source>/, load the impl, engine main(SOURCE)

--------------------------------------------------------------------------
On-disk layout (per source root sources/<name>/)
--------------------------------------------------------------------------
  process-data/session_status.json   live status (same shape as raw phase,
                                     with "work-log" instead of
                                     "network-requests")
  process-data/sessions/<id>/        session.txt, log, archived status
  process-data/process_data_impl.py  the source's implementation
  process-data/processing/           durable intermediary helper scripts —
                                     versioned code, never per-session, never
                                     data files
  data/raw/<ts>/{request,response}/  network requests made DURING processing
                                     (e.g. media downloads) — same request-
                                     record convention as the raw phase
  data/work/                         disposable scratch (unpack dirs, temp
                                     conversions); deletable at any time and
                                     cleared by --abandon-recent-failed
  data/processed/                    the processed output of the source:
                                     processed.sqlite (ALL metadata) +
                                     asset/<kind>/... (all bytes, referenced
                                     by assets.locator; conventions in
                                     common/process-data/processing/
                                     schema-core.sql)

Phase specifics on top of the engine:
  * record list  "work-log" of {ts, type} — network requests via
    session.request_dir(type) (folder created), non-network work steps via
    session.record(utcstamp(), type) (no folder).
  * abandon      additionally clears data/work/. Processed outputs a failed
    session already wrote are NOT rolled back; processing must be idempotent
    with respect to data/processed/.

See session_engine.py's docstring for the full shared contract.
"""

from __future__ import annotations

import os
import shutil
import sys

sys.path.insert(0, os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "shared"))

from session_engine import (  # noqa: E402
    DataSource, Session, Throttle, utcnow, utcstamp,
)
from session_engine import main as _main  # noqa: E402

__all__ = ["ProcessDataSource", "Session", "Throttle", "main",
           "utcnow", "utcstamp"]


class ProcessDataSource(DataSource):
    phase_dir = "process-data"
    records_key = "work-log"
    cli = "process-data.sh"

    def abandon_extra(self, failed_status: dict) -> None:
        work = os.path.join("data", "work")
        if os.path.isdir(work):
            for entry in os.listdir(work):
                if entry == ".gitkeep":
                    continue
                p = os.path.join(work, entry)
                shutil.rmtree(p) if os.path.isdir(p) else os.remove(p)


def main(source: ProcessDataSource, argv) -> int:
    return _main(source, argv)
