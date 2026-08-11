#!/usr/bin/env python3
"""Kaggle 'pavankalyand515/data-telugu-handwritten' zip (~353 MB).

Needs a Kaggle API token (kaggle.com -> Settings -> API); the token is a
credential, NOT a scope input variable:
  Interactive:  ./get-raw-data/get-raw-data.sh            (prompts)
  Pipeable:     echo "$TOKEN" | ./get-raw-data/get-raw-data.sh
                KAGGLE_API_TOKEN=... ./get-raw-data/get-raw-data.sh
  (--bg requires KAGGLE_API_TOKEN in the environment or an interactive prompt
   before detaching; the token is inherited by the background child.)

Scope: no source-specific input variables ({}).
Continue protocol: cursor.leftover_position stores the data/raw/<ts> request
whose response/ holds the partial zip; a retry resumes it with curl -C -.
Fallback: re-initiate the entire download.
"""

import getpass
import os
import subprocess
import sys
import time

from get_raw_data import RawDataSource  # noqa: E402

DL_URL = ("https://www.kaggle.com/api/v1/datasets/download/"
          "pavankalyand515/data-telugu-handwritten")
FILE = "kaggle-data-telugu-handwritten.zip"


class KaggleSource(RawDataSource):
    name = "kaggle-data-telugu-handwritten"
    usage = "[--bg] [--abandon-recent-failed]"

    def prepare(self, argv):
        # Credential, not scope: acquire the token BEFORE any --bg detach so
        # the background child inherits it via the environment.
        if not os.environ.get("KAGGLE_API_TOKEN"):
            if sys.stdin.isatty():
                os.environ["KAGGLE_API_TOKEN"] = getpass.getpass(
                    "Kaggle API token: ")
            else:
                os.environ["KAGGLE_API_TOKEN"] = sys.stdin.readline().strip()
        if not os.environ.get("KAGGLE_API_TOKEN"):
            print("error: no token provided", file=sys.stderr)
            return 1
        return None

    def parse_scope(self, argv):
        if argv:
            raise ValueError(f"unexpected arguments: {' '.join(argv)} "
                             "(this source has no input variables)")
        return {}

    def run(self, session):
        self._download(session, session.request_dir("download"), fresh=True)

    def resume(self, session, position):
        dest = os.path.join("data", "raw", str(position), "response", FILE)
        if isinstance(position, str) and os.path.isfile(dest):
            session.log(f"retry: resuming partial download in data/raw/"
                        f"{position}/response/ (curl -C -)")
            session.record(position, "download")
            self._download(session, position, fresh=False)
        else:
            session.log("retry: leftover position unusable — fallback: "
                        "re-initiating the entire download")
            self.run(session)

    def fallback_position(self, session, previous_requests):
        for req in reversed(list(previous_requests)):
            ts = req.get("ts", "")
            if (req.get("type") == "download" and os.path.isfile(
                    os.path.join("data", "raw", ts, "response", FILE))):
                return ts
        return None  # single-download source: re-initiate the whole download

    def _download(self, session, ts, fresh):
        dest = os.path.join("data", "raw", ts, "response", FILE)
        if fresh:
            with open(os.path.join("data", "raw", ts, "request",
                                   "request.txt"), "w", encoding="utf-8") as f:
                f.write(f"time: {time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())}\n"
                        f"session: {session.session_id}\ntype: download\n"
                        f"method: GET\nprotocol: curl -L --fail --continue-at -\n"
                        f"auth: Bearer <KAGGLE_API_TOKEN> (redacted)\n"
                        f"url: {DL_URL}\noutput: response/{FILE}\n")
        session.set_position(ts)
        session.log(f"Downloading (~353 MB) -> {dest}")
        rc = subprocess.call(["curl", "-L", "--fail", "--continue-at", "-",
                              "-H", f"Authorization: Bearer "
                                    f"{os.environ['KAGGLE_API_TOKEN']}",
                              "-o", dest, DL_URL])
        if rc != 0:
            print("error: download failed", file=sys.stderr)
            raise SystemExit(1)
        session.log("Done.")
        session.set_position(None)


SOURCE = KaggleSource()
