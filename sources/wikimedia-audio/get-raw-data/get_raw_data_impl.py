#!/usr/bin/env python3
"""Wikimedia Commons Telugu audio source.

Scope (source-specific input variables): {"complexity": N}
  complexity 0 (default): fetch the small hardcoded list of KEY audio files
      (one download per file from upload.wikimedia.org). Continue protocol:
      leftover_position = {"files_done": [...]} — resume with the remaining
      files. Fallback: reconstruct files_done from the failed session's
      recorded download requests (data/raw/<ts>/request/request.txt names the
      file; the response file's presence confirms it).
  complexity 1: the full recorder sweep — enumerate every upload of the known
      Telugu recorders via the Commons allimages API (responses stored under
      data/raw/ ARE the acquisition record; audio download happens later in
      processing via --fetch-audio). Continue protocol: leftover_position =
      {"recorders_done": [...], "recorder": <in progress>, "aicontinue": ...}.
      Fallback: read the failed session's api-allimages request folders —
      the last one's aiuser is the recorder it stopped at and its
      response.json "continue.aicontinue" is the page to resume from.
  complexity >= 2 (ring expansion) is rejected.

Helper mode (NO session, used by processing to fetch one matched file):
  get_raw_data_impl.py --fetch-audio <url> <dest-name>
      Downloads through the same shared throttle into a standalone
      data/raw/<ts>/{request,response}/ folder (not associated with any
      session; it appears in no network-requests list) and prints the dest
      path. Exit: 0 ok, 8 rate-limited, 1 failed.

Rate limiting: the Commons API is polite (maxlag honored); the real constraint
is upload.wikimedia.org CDN throttling (429 + long cooldowns), so every
network call goes through the shared flock token bucket at
get-raw-data/.throttle and 429 Retry-After penalties are shared with any
sibling process.
"""

import hashlib
import json
import os
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

from get_raw_data import RawDataSource, Throttle, utcstamp  # noqa: E402

UA = ("TLU-lexicon-coverage/1.0 (Replit workspace research crawler; "
      "telugu pronunciation audit) python-urllib")
MAXLAG = int(os.environ.get("WIKI_MAXLAG", "5"))
INTERVAL = float(os.environ.get("THROTTLE_INTERVAL", "3"))
THROTTLE_PATH = os.path.join("get-raw-data", ".throttle")
AUDIO_EXT = (".wav", ".ogg", ".oga", ".flac", ".mp3", ".opus")

# Full recorder set behind confirmed Telugu pronunciation files (external
# consultation + LinguaLibre/Te- convention recorders of Telugu).
RECORDERS = ["Kasyap", "Navaneethakrishnan.P", "Sriphani", "Veeven", "V Bhavya"]

# complexity 0: key audio files (names from the recordings ledger; URLs follow
# the Commons md5-hash upload scheme, computed below).
KEY_FILES = [
    "LL-Q8097 (tel)-V Bhavya-అ.wav",
    "LL-Q8097 (tel)-V Bhavya-అం.wav",
    "LL-Q8097 (tel)-V Bhavya-అంక.wav",
]


def commons_url(name):
    n = name.replace(" ", "_")
    h = hashlib.md5(n.encode("utf-8")).hexdigest()
    return (f"https://upload.wikimedia.org/wikipedia/commons/"
            f"{h[0]}/{h[:2]}/{urllib.parse.quote(n)}")


DEBUG = open("/tmp/wikimedia-audio-debug.log", "a", encoding="utf-8")


def dbg(msg):
    DEBUG.write(f"{time.strftime('%Y-%m-%d %H:%M:%S')} {msg}\n")
    DEBUG.flush()


throttle = Throttle(THROTTLE_PATH, INTERVAL)  # cwd = source root (run.py chdirs before import)


def _standalone_request_dir():
    """data/raw/<ts>/ folder for a request outside any session (helper mode)."""
    base, ts, n = utcstamp(), utcstamp(), 0
    while os.path.exists(os.path.join("data", "raw", ts)):
        n += 1
        ts = f"{base}-{n:03d}"
    os.makedirs(os.path.join("data", "raw", ts, "request"))
    os.makedirs(os.path.join("data", "raw", ts, "response"))
    return ts


def _write_request_txt(ts, lines):
    with open(os.path.join("data", "raw", ts, "request", "request.txt"),
              "w", encoding="utf-8") as f:
        f.write(f"time: {time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())}\n")
        f.write(lines)


def api(session, purpose, **params):
    """Throttled, logged Commons API GET. Stores request+response under
    data/raw/<ts>/ recorded in the session's network-requests."""
    params.update(action="query", format="json", formatversion=2,
                  maxlag=MAXLAG)
    qs = urllib.parse.urlencode(params)
    url = f"https://commons.wikimedia.org/w/api.php?{qs}"
    for attempt in range(8):
        throttle.take()
        try:
            req = urllib.request.Request(url, headers={"User-Agent": UA})
            with urllib.request.urlopen(req, timeout=30) as r:
                body = r.read()
            d = json.loads(body)
            if d.get("error", {}).get("code") == "maxlag":
                dbg("maxlag, backing off 5s")
                throttle.penalize(5)
                continue
            ts = session.request_dir("api-allimages")
            _write_request_txt(ts, "host: commons.wikimedia.org\nmethod: GET\n"
                               f"purpose: {purpose}\nsession: {session.session_id}\n"
                               f"maxlag: {MAXLAG}\nuser-agent: {UA}\nparams:\n"
                               + "".join(f"  {k}: {v}\n" for k, v in params.items()))
            with open(os.path.join("data", "raw", ts, "response",
                                   "response.json"), "wb") as f:
                f.write(body)
            return d
        except urllib.error.HTTPError as e:
            if e.code == 429 and attempt < 7:
                wait = int(e.headers.get("Retry-After") or 0) or 5 * (attempt + 1)
                dbg(f"api 429, waiting {wait}s (shared penalty)")
                throttle.penalize(wait)
                time.sleep(wait)
            elif attempt == 7:
                raise
            else:
                time.sleep(3 * (attempt + 1))
        except Exception:
            if attempt == 7:
                raise
            time.sleep(3 * (attempt + 1))


def download(url, name, purpose, session=None):
    """Throttled, logged upload.wikimedia.org GET into a fresh data/raw/<ts>/
    (session-recorded when a session is given). Returns (ts, dest) or raises;
    SystemExit(8) when rate limiting could not be outwaited."""
    for attempt in range(8):
        throttle.take()
        try:
            req = urllib.request.Request(url, headers={"User-Agent": UA})
            with urllib.request.urlopen(req, timeout=120) as r:
                body = r.read()
            ts = (session.request_dir("download") if session
                  else _standalone_request_dir())
            _write_request_txt(
                ts, "host: upload.wikimedia.org\nmethod: GET\n"
                f"purpose: {purpose}\n"
                + (f"session: {session.session_id}\n" if session
                   else "session: (none — standalone --fetch-audio helper)\n")
                + f"file: {name}\nuser-agent: {UA}\nurl: {url}\n"
                f"output: response/{name}\n")
            dest = os.path.join("data", "raw", ts, "response", name)
            with open(dest, "wb") as f:
                f.write(body)
            return ts, dest
        except urllib.error.HTTPError as e:
            if e.code == 429 and attempt < 7:
                wait = int(e.headers.get("Retry-After") or 0) or 60 * (attempt + 1)
                dbg(f"download 429, waiting {wait}s (shared penalty)")
                throttle.penalize(wait)
                time.sleep(min(wait, 600))
            elif e.code == 429:
                raise SystemExit(8)
            elif attempt == 7:
                raise
            else:
                time.sleep(3 * (attempt + 1))
        except Exception:
            if attempt == 7:
                raise
            time.sleep(3 * (attempt + 1))


def word_of(filename):
    """Strict convention check (in-memory only; decides log counters):
      Te-<word>.<ext> (optional trailing digit) |
      LL-Q#### (lang)-<recorder>-<word>.<ext>"""
    stem = filename.split(":", 1)[-1].replace("_", " ").rsplit(".", 1)[0]
    m = re.fullmatch(r"Te-([\u0C00-\u0C7F]+)\d*", stem)
    if m:
        return m.group(1)
    m = re.fullmatch(r"LL-Q\d+ \([a-z]+\)-.+?-([\u0C00-\u0C7F]+)", stem)
    if m:
        return m.group(1)
    return ""


class WikimediaAudioSource(RawDataSource):
    name = "wikimedia-audio"
    usage = "[--bg] [--abandon-recent-failed] [--complexity=N]"

    def prepare(self, argv):
        # Session-less side mode: fetch one audio file into a standalone
        # data/raw/<ts>/ folder (no session, no status). Short-circuits the
        # engine entirely.
        if argv and argv[0] == "--fetch-audio":
            if len(argv) != 3:
                print("usage: --fetch-audio <url> <dest-name>",
                      file=sys.stderr)
                return 2
            url, name = argv[1], argv[2]
            try:
                ts, dest = download(url, name, f"matched-audio fetch {name}")
            except SystemExit as e:
                return e.code if isinstance(e.code, int) else 1
            except Exception as e:
                print(f"error: {e}", file=sys.stderr)
                return 1
            print(dest)
            return 0
        return None

    def parse_scope(self, argv):
        complexity = 0
        for a in argv:
            if a.startswith("--complexity="):
                try:
                    complexity = int(a.split("=", 1)[1])
                except ValueError:
                    raise ValueError(f"bad complexity: {a}")
            else:
                raise ValueError(f"unexpected argument: {a}")
        if complexity not in (0, 1):
            raise ValueError("complexity >= 2 (ring expansion) is not defined "
                             "for this source; use 0 (key files) or 1 "
                             "(recorder sweep)")
        return {"complexity": complexity}

    def run(self, session):
        if session.scope["complexity"] == 0:
            self._c0(session, {"files_done": []})
        else:
            self._c1(session, {"recorders_done": [], "recorder": None,
                               "aicontinue": None})

    def resume(self, session, position):
        c = session.scope["complexity"]
        if c == 0 and isinstance(position, dict) and "files_done" in position:
            session.log(f"retry: resuming key files, "
                        f"{len(position['files_done'])} already fetched")
            self._c0(session, position)
        elif c == 1 and isinstance(position, dict) and "recorders_done" in position:
            session.log(f"retry: resuming recorder sweep at "
                        f"{position.get('recorder') or 'next recorder'}")
            self._c1(session, position)
        else:
            session.log("retry: leftover position unusable — starting from "
                        "the beginning")
            self.run(session)

    def fallback_position(self, session, previous_requests):
        c = session.scope["complexity"]
        if c == 0:
            done = []
            for req in previous_requests:
                ts = req.get("ts", "")
                if req.get("type") != "download":
                    continue
                rt = os.path.join("data", "raw", ts, "request", "request.txt")
                try:
                    txt = open(rt, encoding="utf-8").read()
                except Exception:
                    continue
                m = re.search(r"^file: (.+)$", txt, re.M)
                if m and os.path.isfile(os.path.join("data", "raw", ts,
                                                     "response", m.group(1))):
                    done.append(m.group(1))
            return {"files_done": done} if done else None
        # c1: last api-allimages request tells us the recorder and, from its
        # stored response, the aicontinue to resume from.
        last = None
        for req in previous_requests:
            if req.get("type") == "api-allimages":
                last = req.get("ts", "")
        if not last:
            return None
        try:
            txt = open(os.path.join("data", "raw", last, "request",
                                    "request.txt"), encoding="utf-8").read()
            user = re.search(r"^  aiuser: (.+)$", txt, re.M).group(1)
            resp = json.load(open(os.path.join("data", "raw", last, "response",
                                               "response.json"),
                                  encoding="utf-8"))
        except Exception:
            return None
        cont = resp.get("continue", {}).get("aicontinue")
        idx = RECORDERS.index(user) if user in RECORDERS else 0
        if cont:
            return {"recorders_done": RECORDERS[:idx], "recorder": user,
                    "aicontinue": cont}
        return {"recorders_done": RECORDERS[:idx + 1], "recorder": None,
                "aicontinue": None}

    # --- complexity 0: key audio files ---------------------------------------
    def _c0(self, session, state):
        done = list(state.get("files_done", []))
        todo = [n for n in KEY_FILES if n not in done]
        session.log(f"key files: {len(KEY_FILES)} total, {len(todo)} to fetch")
        for name in todo:
            url = commons_url(name)
            ts, dest = download(url, name, f"key-file download {name}",
                                session=session)
            done.append(name)
            session.set_position({"files_done": done})
            session.log(f"  fetched {name} -> {dest}")
        session.set_position(None)
        session.log(f"done: all {len(KEY_FILES)} key files present")

    # --- complexity 1: full recorder sweep ------------------------------------
    def _c1(self, session, state):
        recorders_done = list(state.get("recorders_done", []))
        total_files = 0
        for user in RECORDERS:
            if user in recorders_done:
                continue
            cont = {}
            if state.get("recorder") == user and state.get("aicontinue"):
                cont = {"aicontinue": state["aicontinue"]}
                session.log(f"  resuming {user} mid-sweep from persisted "
                            f"aicontinue")
            parsed = unparsed = audio = 0
            while True:
                d = api(session, f"recorder sweep {user}",
                        list="allimages", aiuser=user, aisort="timestamp",
                        aiprop="timestamp", ailimit="500", **cont)
                for fobj in d.get("query", {}).get("allimages", []):
                    n = fobj["name"]
                    if not n.lower().endswith(AUDIO_EXT):
                        continue
                    audio += 1
                    if word_of(n):
                        parsed += 1
                    else:
                        unparsed += 1
                if "continue" not in d:
                    break
                cont = {"aicontinue": d["continue"]["aicontinue"]}
                session.set_position({"recorders_done": recorders_done,
                                      "recorder": user,
                                      "aicontinue": cont["aicontinue"]})
            recorders_done.append(user)
            session.set_position({"recorders_done": recorders_done,
                                  "recorder": None, "aicontinue": None})
            total_files += audio
            session.log(f"  {user}: {audio} audio files enumerated "
                        f"({parsed} parsed / {unparsed} unparsed)")
        session.set_position(None)
        session.log(f"done: {len(RECORDERS)} recorders swept, "
                    f"{total_files} audio files enumerated this run")


SOURCE = WikimediaAudioSource()
