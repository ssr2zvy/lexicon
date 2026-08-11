#!/usr/bin/env python3
"""Sketch Engine exports (manual: the corpus UI requires a logged-in session;
there is no public download URL). Documents the export, then ingests and
verifies the CSVs.
  Interactive:  ./get-raw-data/get-raw-data.sh    (instructions, wait, verify)
  Pipeable:     echo /path/to/downloads | ./get-raw-data/get-raw-data.sh

Scope: no source-specific input variables ({}).
Continue protocol: none is discernible for a manual import, so a retry always
uses the fallback mechanism — re-initiate the entire import.
"""

import glob
import os
import shutil
import sys
import time

from get_raw_data import RawDataSource  # noqa: E402

INSTRUCTIONS = """Manual export from Sketch Engine (corpus: Telugu Web 2021, teTenTen21):
  1. app.sketchengine.eu -> select the teTenTen21 corpus
  2. Wordlist  -> attribute "word", download CSV        (wordlist_*.csv)
  3. Wordlist  -> attribute "word" grouped by POS, CSV  (poswordlist_*.csv)
  4. N-grams   -> 2-grams and 3-grams, download CSV     (ngrams_*.csv)
Pipe a directory path to this script to copy every *.csv from it, or place
the CSVs by hand and press Enter when prompted."""


class SketchEngineSource(RawDataSource):
    name = "sketch-engine-export"
    usage = "[--bg] [--abandon-recent-failed]"

    def parse_scope(self, argv):
        if argv:
            raise ValueError(f"unexpected arguments: {' '.join(argv)} "
                             "(this source has no input variables)")
        return {}

    def run(self, session):
        print(INSTRUCTIONS)
        ts = session.request_dir("manual-import")
        session.set_position(ts)
        dest = os.path.join("data", "raw", ts, "response")
        srcdir = ""
        if not sys.stdin.isatty():
            srcdir = sys.stdin.readline().strip()
            if srcdir and os.path.isdir(srcdir):
                for p in glob.glob(os.path.join(srcdir, "*.csv")):
                    shutil.copy(p, dest)
                    session.log(f"copied {p}")
        else:
            try:
                input(f"Press Enter once the CSVs are in {dest} ... ")
            except EOFError:
                pass
        with open(os.path.join("data", "raw", ts, "request",
                               "request.txt"), "w", encoding="utf-8") as f:
            f.write(f"time: {time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())}\n"
                    f"session: {session.session_id}\ntype: manual-import\n"
                    "protocol: manual export from app.sketchengine.eu "
                    "(logged-in UI), CSVs copied in\n"
                    f"source-directory: {srcdir or '(placed by hand)'}\n"
                    "output: response/*.csv\n")
        n = len(glob.glob(os.path.join(dest, "*.csv")))
        if n == 0:
            print(f"error: no CSVs found in {dest}", file=sys.stderr)
            raise SystemExit(1)
        session.log(f"OK: {n} CSV file(s) in {dest}")
        session.set_position(None)

    def resume(self, session, position):
        session.log("retry: manual import has no resumable position — "
                    "fallback: re-initiating the entire import")
        self.run(session)

    def fallback_position(self, session, previous_requests):
        return None  # always re-initiate the entire import


SOURCE = SketchEngineSource()
