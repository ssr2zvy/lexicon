"""process_data_impl.py — sketch-engine-export processing implementation on the shared
framework (common/process_data.py). Run via:
    common/process-data.sh sketch-engine-export [--bg] [--abandon-recent-failed]

SKELETON: the session/resume machinery is wired, but run()/resume()/
fallback_position() are pending the shared SQL schema decision for
data/processed/ — invoking this exits 3 without doing any work."""

from __future__ import annotations

import os
import sys

from process_data import ProcessDataSource  # noqa: E402


class Impl(ProcessDataSource):
    name = "sketch-engine-export"
    usage = "[--bg] [--abandon-recent-failed]"

    def parse_scope(self, argv):
        if argv:
            raise ValueError(f"unknown arguments: {' '.join(argv)}")
        return {}

    def run(self, session):
        session.log("not implemented yet: processing for sketch-engine-export is pending the "
                    "shared SQL schema decision")
        raise SystemExit(3)

    def resume(self, session, position):
        self.run(session)

    def fallback_position(self, session, previous_records):
        return None


SOURCE = Impl()
