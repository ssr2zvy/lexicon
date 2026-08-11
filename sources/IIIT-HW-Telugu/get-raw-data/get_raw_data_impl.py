#!/usr/bin/env python3
"""IIIT-HW-Telugu handwriting dataset (single large tar.gz).

Scope: no source-specific input variables ({}).
Continue protocol: cursor.leftover_position stores the data/raw/<ts> request
whose response/ holds the partial file; a retry resumes it with curl -C -.
Fallback: re-initiate the entire download.
"""

import os
import subprocess
import sys
import time

from get_raw_data import RawDataSource  # noqa: E402

URL = ("https://cvit.iiit.ac.in/images/Projects/wordlevel-Indicscripts/"
       "IIIT-HW-Telugu_v1.tar.gz")
FILE = "IIIT-HW-Telugu_v1.tar.gz"
CURL = ["curl", "--continue-at", "-", "--location", "--max-time", "120",
        "--speed-limit", "1", "--speed-time", "60", "--progress-bar"]


class IIITSource(RawDataSource):
    name = "IIIT-HW-Telugu"
    usage = "[--bg] [--abandon-recent-failed]"

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
                        f"method: GET\nprotocol: {' '.join(CURL)}\n"
                        f"url: {URL}\noutput: response/{FILE}\n")
        session.set_position(ts)
        for attempt in range(1, 6):
            session.log(f"[{time.strftime('%Y-%m-%d %H:%M:%S')}] "
                        f"Attempt #{attempt} — resuming download...")
            rc = subprocess.call(CURL + ["--output", dest, URL])
            if rc == 0:
                session.log(f"Download complete: {dest}")
                if subprocess.call(["tar", "-tzf", dest],
                                   stdout=subprocess.DEVNULL,
                                   stderr=subprocess.DEVNULL) == 0:
                    session.log("Archive is valid. Done.")
                    session.set_position(None)
                    return
                session.log("WARNING: archive corrupt; removing and retrying "
                            "from scratch.")
                os.remove(dest)
            else:
                session.log(f"curl exited with code {rc}; retrying in 10s...")
            time.sleep(10)
        print("error: download did not complete after 5 attempts",
              file=sys.stderr)
        raise SystemExit(1)


SOURCE = IIITSource()
