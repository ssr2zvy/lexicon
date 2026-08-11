"""get_raw_data.py — the get-raw-data phase FAÇADE. __all__ below is the
ENTIRE public contract between the framework and the source impls
(sources/<name>/get-raw-data/get_raw_data_impl.py): impls import ONLY this
module, and only these names; the engine (common/shared/session_engine.py)
is private and has exactly two importers — this file and its process-data
twin. One legal import path per symbol, declared, so an engine refactor
that keeps this surface intact provably breaks nothing.

Entry flow (single entry point, no exceptions):
  common/get-raw-data/get-raw-data.sh <source> [options...]
    -> exec common/get-raw-data/run.py <source> [options...]
       -> chdir sources/<source>/, load the impl, engine main(SOURCE)

Phase specifics on top of the engine:
  * phase dir       get-raw-data/   (session_status.json, sessions/<id>/)
  * record list     "network-requests" — every network request creates a
                    data/raw/<ts>/{request,response}/ folder via
                    session.request_dir(type); type is "download" for
                    downloads or the API call's kind (e.g. "api-allimages",
                    "player", "browse", "subtitle", "manual-import").
  * abandon         deleting the failed session removes its recorded
                    data/raw/<ts> folders, its sessions/<id>/ record and the
                    live status (no extra cleanup).

See session_engine.py's docstring for the full shared contract
(--bg / --abandon-recent-failed, new|retry lifecycle, status shape,
prepare/resume/fallback drive, truthful finalization).
"""

from __future__ import annotations

import os
import sys

sys.path.insert(0, os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "shared"))

from session_engine import (  # noqa: E402
    DataSource, Session, Throttle, utcnow, utcstamp,
)
from session_engine import main as _main  # noqa: E402

__all__ = ["RawDataSource", "Session", "Throttle", "main",
           "utcnow", "utcstamp"]


class RawDataSource(DataSource):
    phase_dir = "get-raw-data"
    records_key = "network-requests"
    cli = "get-raw-data.sh"


def main(source: RawDataSource, argv) -> int:
    return _main(source, argv)
