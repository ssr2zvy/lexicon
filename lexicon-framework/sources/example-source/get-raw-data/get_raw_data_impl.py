#!/usr/bin/env python3
"""YouTube human Telugu captions source.

Scope (source-specific input variables): {"complexity": N}
  complexity 0 (default): player-check the hardcoded seed videos; store the
      human (non-ASR) Telugu subtitle of every verified video; record the
      seeds' channels.
  complexity 1: additionally sweep every discovered channel's Videos tab and
      check/store subtitles for all of its videos.
  complexity >= 2 is rejected (undefined).

Continue protocol: leftover_position =
  {"seeds_done": [...], "channels_found": [...], "channels_done": [...],
   "videos_checked": [...], "videos_verified": [...]}
Fallback (no discernible position): reconstruct it from the failed session's
recorded requests — compare the "track check <vid>" player purposes against
the hardcoded seed list (c0), and use the "channel videos <ch>" browse
purposes to find the current channel and the remainder (c1; the last-seen
channel is re-swept, videos_checked prevents re-checking its videos).

Every network call goes through the shared flock token bucket at
get-raw-data/.throttle; 429 Retry-After penalties are shared with siblings.
A YouTube bot-wall (LOGIN_REQUIRED) aborts with rc 3 — run_status=fail with
the cursor saved, so a later rerun retries from where it stopped.
"""

import gzip
import json
import os
import re
import sys
import time
import urllib.error
import urllib.request

from get_raw_data import RawDataSource, Throttle  # noqa: E402

INTERVAL = float(os.environ.get("THROTTLE_INTERVAL", "1.5"))
PAGE_CAP = int(os.environ.get("CHANNEL_PAGE_CAP", "5"))
THROTTLE_PATH = os.path.join("get-raw-data", ".throttle")

UA_ANDROID = "com.google.android.youtube/20.10.38 (Linux; U; Android 11) gzip"
UA_WEB = ("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
          "(KHTML, like Gecko) Chrome/124.0 Safari/537.36")
CTX = {"context": {"client": {"clientName": "ANDROID",
                              "clientVersion": "20.10.38",
                              "androidSdkVersion": 30, "hl": "en"}}}
WEB_CTX = {"context": {"client": {"clientName": "WEB",
                                  "clientVersion": "2.20250101.00.00",
                                  "hl": "en"}}}
VIDEOS_TAB = "EgZ2aWRlb3PyBgQKAjoA"  # channel "Videos" tab params

# The dataset's input: videos externally discovered (prior exploratory crawl,
# each confirmed at discovery time to carry a human non-ASR Telugu subtitle
# track). Their channels are the ring-0 expansion surface.
SEED_VIDEOS = """
-8EJ9uhsfQQ -K-bhRczNbc -tENnGjU-FU -tRQqmcNdTM -uyltCF1bfo 0PHddY7SZrg 0_trSrXF7CI 0cpmhxzvXUc
0gWsuqVv1Ps 0ljGZXoO_oE 1-b4zrz3UYY 10CEDrDrl5g 16turBpaJco 18cozHujbiU 1as7970jNu4 1f082NwVkQE
1fNWTZZwgbs 1mm8BbIHXjI 227QaFeJidg 23SNXz0UVFk 2H17HWYR5iU 2PMu3ULezxU 2P_0Q8CIjw8 2nNdj90MCAg
3EBktAo6LUI 3KfqM5DPHro 3Nu5oJtfc9o 3xDjCukiNUg 4Pp5mWs8k5I 4c1MrRrepec 4osdcR9AJxk 4rfmKR9XEA0
4xzGLYOG2tg 5AMgZUJZSDc 5CBxwrd0O-0 5sW3A3sp0BA 5yjy7djTUDc 62qLt6X1AK0 6Fk7Vum_CaQ 6GWQ6VcBMn4
6Ld5VjXaGc0 6ifd7hGS-54 6qaoIhu86Ck 6tXD2e9n-S8 6wJBGsM-J-Y 6wqwB62-Sk4 7EYlydFMIUo 7Ug1jPzaJrw
7XuUbXqE--o 7blxf1pLlGk 7f9YyLlVg1s 8-RUQE7uzgQ 87uJYE8DnmY 8AhgmklKFQs 8VIPgGUIq5U 8Wi01h9tolU
8b6QgQdg5BI 974CxR-AzTQ 997GgkpTbL8 9DwzO_zRu5g 9LindFpzy6Q 9MYWuWJNSBg 9QHOO53KnGA 9ZLeYNnIPmQ
9dckjVqFsHo 9izjgIcO7EY 9phqtkCM97Q 9pxmjNE_eY4 9zjmZO63I3o A-XzUYHrJBw AIT57XBP2Lc AQiOjXI0Cpc
ARao8oKv0g8 ASCzKRlZqn4 A_z5g0_hJN8 AafFuA7NP0g BHr7F3ya70A BLXEw2LFbwQ BOwr3jXZkGY BQE2hrC_gFo
BcjlMecH8KA Bcs21oEu6_w BxV0vTS1Zq8 CBXTnnecjK4 CESvxypLnnQ CN4wL_cZDfU Cgo3JNTlQa0 Cknmq34vh88
CtfFQGVGbiA CuorQPprjJo D1w2UFsHKls DFX98VFW7q0 DQetkn4SutE DSrrICGjJkc DYTkwPfW_0c DfmZHEg4nU8
DjDP3q5ugPM DmqniUyVlkM DrNB8Aw-VcM DuIplN_Ie5o DxDF8fuexLw E4ZwJTA3z7k E8UQeqA4EfI E9IxQuZNihE
EDnlQrzWEZQ EThWWliR63w EUXIbm42E3c EZrv-TUpfno EkDi5Zruaw0 En1GjWz5yV4 EtxenufUJKY EuCwl-przdk
Ew6ehUCAKXc F306GV4VSyA FUeRf4LDm9k FVPn89mAcb4 FYB0X-vSxWY F_IcVWBoBdY FiW73_Z0vkM FtHyPWMuMiI
FwuS_3HLuu4 FzLpP8VBC6E GL94E1K5gXA GNcfytlI07Q GRTIkvBtDJY GWGQhAH6d34 GWH_k6S7YQU GjD9mDlL3MU
GkjwNzgbvh8 GlxWFLe_cks GsCBX_t4Zw4 H7Osnkf4ZY4 HIN2IDOofbM HJLrmymYHmk HN-F3uJfUhE HZZYPyoyWvU
HbQrcZCP-DU HnPkqsrEA0g HqlmZaNqlbs Hy_MCm76aGo I2tmfCbtY5U I7T0tfieOf0 I8UrKhurkuk INRAvyoRSfg
IvUt8AqFEg8 Iw77lXHWGsA IwflP_a2tJU J61u6MfKJKE JEEJTg8mt38 JGst78efok0 JM894qZpYU0 JMNXpTXec24
JdnSl6JVIIM JgJR2Hx0yCg JizdAP_j3kY JympswqpmvQ K43uELZscbk K4SPlVM5WCU K5J1rX3lgPM KH0s-oe3FrI
KIz2zVK8lqI KU4rAtWD7yk KelvvJpHca0 KhdrpKFUPm8 KlIVWKxaQ4g KqZvavNzUps KrCpa1zJpzE KtFfG3Arq4s
KuSR4-cVb30 L1fX3TmwQEY LbHdQHdKsYU LdQRRsVKRRc LiDvNRTzvDU LkVVwptf4Aw MAsWqb5kdUc MTvV47ud7tk
Mh02YSTO3mc Mh3GI0PosIA MpWrpB_x5Cc Ncbd_j7Vab4 NsFZK531wNg NtRZGoHFRlU OHs8Z2ApKk0 OTLdpLd7ORM
OUi3DKcI1VY OYvqfM2G0fk OdQS_wJJ_AM OfqPV1ds1tw Ol1l7QUFFNU OxurEP8CfS4 PFeOxepY-cw PIpw6BNCcAc
PNbZ9CaUwGE PciVUiE208E Pee13mBWKpM PiECSnjgrYo PsztAZYjVKk Q69IgPG1qng QSs4NUv4eSU QVDf0wvSANU
QVXiyXM83OM Qb0opOAk2LM QivI9HxsUVY QnuBfuEEjiU Qtorvu1VuK4 R9trTrcpNpY RACv-Sm_BxE RFgsYAiPqfQ
RJrQqLqC1sM RLHaDkynThA RYhirK3ip8s RZ0A5eLsATg RmSdum4BMqI ScVcWaU2FUA SdcAN3dobz4 SdimH8muyyo
SjLi4rAGP9k SjsXrNMmzwY SnQ6OiGOhtg T7MKkX9lYsA TPM49DqjwFw TW5vdwUzWVM TZ0yTDAKDMc T_5LumvWeo8
Tb9xK8Byu1M TgBYIyeOeAU TotzvlxbzDI U5LQtvn2rjQ U9drpRPHCjg UA2cTZAWS00 UIwaJb6Ce-4 UMBO0m0XEjo
UX_v7favdWA U_RWHjYThik UdZzW6QzN-s V37grBrHKGg V9Fh3jqMZTk VSE6qM8KDFQ ViuA8lYnKhM VvEUEpEKpyQ
W10-Xwk_fsU W4FRzLwIzTo WP5zY4tWv7s W_ESDuo9kEU X2ofxziuYyo X4_Ey4pLLiw XAmQ9yO1MY0 XF0zu-lDLew
XbxW7ommC9U XdjgaANXAHs XeGGdyPHXE4 XoLfStLHIOQ XtKSm5ntzAA Y963o_1q71M YQ4tcO1_nms YQn6zm9LScA
YW9Pi2kHo1Q YY-8XOi7G5k YY2mmoPWjPk YdiweEPWUwo Ynatzy4Dv4E Yu_-4cfOhKo Yxo0-RvOwA4 Z-8l73O0CwA
Z06G2jmMn0I Z7w3ULLoV_A Z8fUFG2TKTw ZAEcQkRQVL4 ZRJm3mSL7Vw ZTmUWSVkcNA Zb5nmnixzYQ ZhADhaXbaGM
ZoRa-JqQ76E Zpy9qMs4ztA _L55wXJHYVo _Onv4UtIxnI _bLR-eV5BFI a4QPclTGobA a6HLT2bVTx8 aQAsCl1Y2JY
aSx2ZE0YLxk b6xcKxKHP-c bXa-wbiXiOw bmjNrbVy5KA bnnF5Rr5VRw bt4-FwVe1Fk bxDsdwl46vA c3QzEI28mtA
cOYTScJbCTA cZQZXZdflWE ca2icA18jNQ cy2OBy39Jmg d-dcHaj-Buc d0A6Uchb1F8 d3QbDvI83N8 dKpv0VWQOCM
dTq7OAwn6to dUwHqa3BOkE dV7-QjWmnns daKaVeMvZ6A dhH6U8wjF60 dpeXvHXmLP8 dqb0mdDHZ1Q dzF94DUUYBE
dzdm5Z6q15E eDVBZ0VUuC8 eW1C0u6hK70 eWKUzsYx340 eb4D0R0SG3g eyLkkH404rE fAPqB9pPh68 fChrsMQ-f4g
fRUovyyHsoo fih0mgzcOt4 fuRkekxSgIc gCftilhuzv4 gDWYbynjMX8 gN7cODH1T1E gQ-yaYIWE9U gYiv0gxerHQ
gz4rD387JmI gzlfA76Wajs h1_kzaRW9VA hHte6y_B04I hbGpuXchfe8 hd9CZSXQL00 ho6zPc2166U hs0LFOtyinE
hwcsdSqbWwo i0nC9EcUQ1Y iEMGLA5cN2g iHWeHi53fy4 iO_ekYr4LyA iSbZ6KqkqzM iXTKRpG3HEg ifPmbt_RnRc
igbxUqEJ90k j2MQ4RJutvM j2SiAzfPr-c j9phNEaPrv8 jI0Qg9MqOgg jQCh45IqxlQ jVhu7ACt68w k2vCjXW7J80
k7VdcsyF9mU k9VQphfpbyo kE6SZ1ogOVU kFUJma_-IL8 kLNaaryjVIY kVPj90uyTVU kiX8xfGgPrI kmWzImt5EPU
knfgHPq2j50 l-0ov1-Wd2k l0Yy-qOU5xY lDjA1nOjxY0 lQbo34OLs3M lQyjxMj8qa4 lX-02kth3OM lZuHHzKdovM
l_Oozx9rLaM ljO8ab-F7XE loIR54mt6Y8 m4ZeaswqZWQ m4rsWaCQ0bA m8UwE799tkU mF7ds5yYnSU mGgWaPGpGz4
mRJhe8DFg3E mUq4szfC1HU mithMK48_T8 mjwUSRIbu0A mpPErzqk7e0 mqAH69ZtWLo msQd6azVRws muXRBjpya6c
mvNMDTWmmzQ nGGmnwQdiuU nT6FBMyOh_E nT6M_FI6Xoo nahGsPv7PVo nngwP1WWva4 nymMxV88QyY o32leXxSzbE
o7c9rdhEjaI oDnzVDch3RQ oGEGUDM9Iwc oNNZO9i1Gjc oPyoLGv_ZYY oiePMSVT-hc ol86GmzBMfQ op0eC_j35io
otMrEUGQJTY ovXE-vdSAL4 p-Bp9JjuYcs p2FF4wj1gZ8 pCV0OfJXxsg pE0lzTXKbgI pEDH6txrbaU pRwT1fyhKMY
pc1YcZ24kVE pheFN3wCZsg piFjiy3ujMs pw3YZafLCds q5NhJ_3xNOo q68ZV9Z-PTQ qKAFfR7uFtg qd16rzf8KTs
qghzIyTtt4c qk-MsRq9PLY qk2Wx6--34Q qu9AfzDDh5c qvxpX9HBvyw r05kgMO_uRE r1jkeU65Cxk rCj3QoXRWhE
rkO2U35SOgs rmw2oqiW4Jw rvYqvMaSYn0 rx1JGUSGGIE rxu8DsF2tZU ryD8BqVexJI sAC5_zF5iE8 sONk1qyJ2BY
sTA45AZntbM sWlWUCAe26Q skK1wDRDkGw sprPEGmCTbE sqoW7HUulaw szlYrUbyzpY t9G7hw5S4yQ tLOt_nnAuSI
tMKBfs-Kd4g tRzdm_3vCcA tdLl3UVRP3g tk-gPN8yVrE u05e8MZY_0I u6V4tgr7qgw u7Izgp7s0e4 uWNuwzxrnlQ
ubPSLrvZDpU uduYFgJXf8o ue14Msw86ho uiEHJT10Nfg utFINi0zmf4 v289avJydoI vEOvxa0u7Ys vWAQhDXJYKg
vXfmJhuWL9Y vbByJyq0HEE vlB2ugBxB6A vy9QO01mbkI w2cuzA-QmoA w2jYag8ba-E wK3-kOEtWeg wK5YbVMXTyw
wbVg7tLAwqM wcUf9qC56Tc wiSDQEF_cks wlZ40F-T174 xTVZmhip11Q xdIRBGjGagk xizl9KsXxsY xpN6pVMfNx8
y71gxbS_ZAw yF-__4P4GXM yKDWXC4o5nA yNyn1Gk4z3A ySE-kQoFhxQ yXJIA7YzRgY yo_icS4RyB0 ywfssuMSmMA
yy-K5geCKvU yyvtWgPwZLw z8pTFSHzjYc zeQXPNT1UlY zltVKbQrJQk zxZQDPjW15o
""".split()

DEBUG = open("/tmp/youtube-captions-debug.log", "a", encoding="utf-8")


def dbg(msg):
    DEBUG.write(f"{time.strftime('%Y-%m-%d %H:%M:%S')} {msg}\n")
    DEBUG.flush()


class BotWall(Exception):
    pass


throttle = Throttle(THROTTLE_PATH, INTERVAL)  # cwd = source root (run.py chdirs before import)


def _fetch(req):
    with urllib.request.urlopen(req, timeout=30) as r:
        raw = r.read()
    return gzip.decompress(raw) if raw[:2] == b"\x1f\x8b" else raw


def _store(session, rtype, purpose, req_lines, body_name, body):
    ts = session.request_dir(rtype)
    with open(os.path.join("data", "raw", ts, "request", "request.txt"),
              "w", encoding="utf-8") as f:
        f.write(f"time: {time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())}\n"
                f"kind: {rtype}\npurpose: {purpose}\n"
                f"session: {session.session_id}\n")
        f.write(req_lines)
    with open(os.path.join("data", "raw", ts, "response", body_name), "wb") as f:
        f.write(body)


def innertube(session, endpoint, rtype, purpose, body, ua):
    """Throttled, logged innertube POST. Returns parsed JSON."""
    data = json.dumps(body).encode()
    url = f"https://www.youtube.com/youtubei/v1/{endpoint}?prettyPrint=false"
    for attempt in range(5):
        throttle.take()
        try:
            raw = _fetch(urllib.request.Request(url, data=data, headers={
                "Content-Type": "application/json", "User-Agent": ua}))
            _store(session, rtype, purpose,
                   f"method: POST\nendpoint: {endpoint}\nuser-agent: {ua}\n"
                   "body:\n" + json.dumps(body, ensure_ascii=False, indent=2)
                   + "\n", "response.json", raw)
            return json.loads(raw)
        except urllib.error.HTTPError as e:
            if e.code == 429 and attempt < 4:
                wait = int(e.headers.get("Retry-After") or 0) or 10 * (attempt + 1)
                dbg(f"429 on {endpoint}, waiting {wait}s (shared penalty)")
                throttle.penalize(wait)
            elif attempt == 4:
                raise
            else:
                time.sleep(3 * (attempt + 1))
        except Exception:
            if attempt == 4:
                raise
            time.sleep(3 * (attempt + 1))


def fetch_subtitle(session, url, purpose):
    """Throttled, logged timedtext GET. Stores the raw XML."""
    for attempt in range(5):
        throttle.take()
        try:
            raw = _fetch(urllib.request.Request(url,
                                                headers={"User-Agent": UA_WEB}))
            _store(session, "subtitle", purpose,
                   f"method: GET\nuser-agent: {UA_WEB}\nurl: {url}\n",
                   "subtitle.xml", raw)
            return raw
        except Exception:
            if attempt == 4:
                raise
            time.sleep(3 * (attempt + 1))


STATE_KEYS = ("seeds_done", "channels_found", "channels_done",
              "videos_checked", "videos_verified")


class YoutubeCaptionsSource(RawDataSource):
    name = "youtube-captions"
    usage = "[--bg] [--abandon-recent-failed] [--complexity=N]"

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
            raise ValueError("complexity >= 2 is not defined for this source; "
                             "use 0 (seed videos) or 1 (+ channel sweeps)")
        return {"complexity": complexity}

    def run(self, session):
        self._crawl(session, {k: [] for k in STATE_KEYS})

    def resume(self, session, position):
        if isinstance(position, dict) and all(k in position for k in STATE_KEYS):
            session.log(f"retry: resuming — {len(position['seeds_done'])}/"
                        f"{len(SEED_VIDEOS)} seeds done, "
                        f"{len(position['channels_done'])} channels done")
            self._crawl(session, position)
        else:
            session.log("retry: leftover position unusable — starting from "
                        "the beginning")
            self.run(session)

    def fallback_position(self, session, previous_requests):
        """Reconstruct from the failed session's recorded requests: player
        'track check <vid>' purposes vs the hardcoded seed list; subtitle
        'transcript of <vid>' purposes; browse 'channel videos <ch>' purposes
        (last-seen channel = the one in progress, re-swept on resume)."""
        checked, verified, channels_seen = [], [], []
        found_any = False
        for req in previous_requests:
            ts, rtype = req.get("ts", ""), req.get("type", "")
            if rtype not in ("player", "browse", "subtitle"):
                continue
            try:
                txt = open(os.path.join("data", "raw", ts, "request",
                                        "request.txt"),
                           encoding="utf-8").read()
            except Exception:
                continue
            found_any = True
            m = re.search(r"^purpose: track check ([\w-]{11})", txt, re.M)
            if m and m.group(1) not in checked:
                checked.append(m.group(1))
            m = re.search(r"^purpose: transcript of ([\w-]{11})", txt, re.M)
            if m and m.group(1) not in verified:
                verified.append(m.group(1))
            m = re.search(r"^purpose: channel videos (\S+) page", txt, re.M)
            if m and m.group(1) not in channels_seen:
                channels_seen.append(m.group(1))
        if not found_any:
            return None
        return {"seeds_done": [v for v in SEED_VIDEOS if v in checked],
                "channels_found": channels_seen,
                "channels_done": channels_seen[:-1],
                "videos_checked": checked,
                "videos_verified": verified}

    # --- the crawl -------------------------------------------------------------
    def _crawl(self, session, state):
        complexity = session.scope["complexity"]
        seeds_done = set(state["seeds_done"])
        channels_found = set(state["channels_found"])
        channels_done = set(state["channels_done"])
        videos_checked = set(state["videos_checked"])
        videos_verified = set(state["videos_verified"])

        def flush():
            session.set_position({
                "seeds_done": sorted(seeds_done),
                "channels_found": sorted(channels_found),
                "channels_done": sorted(channels_done),
                "videos_checked": sorted(videos_checked),
                "videos_verified": sorted(videos_verified)})

        def check_video(vid, why):
            """Player call; if a human te track exists, fetch+store its
            subtitle. Returns channelId (or None); raises BotWall."""
            player = innertube(session, "player", "player",
                               f"track check {vid} ({why})",
                               {**CTX, "videoId": vid}, UA_ANDROID)
            status = player.get("playabilityStatus", {}).get("status", "")
            if status == "LOGIN_REQUIRED":
                raise BotWall(player.get("playabilityStatus", {})
                              .get("reason", status))
            videos_checked.add(vid)
            details = player.get("videoDetails", {})
            title = details.get("title", "")
            channel = details.get("channelId", "")
            tracks = (player.get("captions", {})
                      .get("playerCaptionsTracklistRenderer", {})
                      .get("captionTracks", []))
            url = next((t["baseUrl"] for t in tracks
                        if t.get("languageCode") == "te"
                        and t.get("kind") != "asr"), None)
            if url:
                fetch_subtitle(session, url,
                               f"transcript of {vid} ({title[:60]})")
                videos_verified.add(vid)
                dbg(f"  {vid}: VERIFIED human te track, subtitle stored "
                    f"({title[:60]})")
            else:
                dbg(f"  {vid}: no human te track ({title[:60]})")
            flush()
            return channel or None

        def channel_video_ids(channel_id):
            """Enumerate a channel's Videos tab (browse + continuations)."""
            ids, cont = [], None
            for page in range(PAGE_CAP):
                body = ({**WEB_CTX, "continuation": cont} if cont
                        else {**WEB_CTX, "browseId": channel_id,
                              "params": VIDEOS_TAB})
                d = innertube(session, "browse", "browse",
                              f"channel videos {channel_id} page {page + 1}",
                              body, UA_WEB)
                blob = json.dumps(d)
                page_ids = re.findall(r'"videoId": ?"([\w-]{11})"', blob)
                ids += page_ids
                dbg(f"  channel {channel_id} p{page + 1}: {len(page_ids)} ids")
                m = re.search(r'"continuationCommand": ?\{"token": ?"([^"]+)"',
                              blob)
                if not m:
                    break
                cont = m.group(1)
            return list(dict.fromkeys(ids))

        dbg(f"--- crawl start (session={session.session_id}, "
            f"complexity={complexity}, seeds={len(SEED_VIDEOS)}) ---")
        try:
            todo_seeds = [v for v in SEED_VIDEOS if v not in seeds_done]
            dbg(f"ring 0: {len(SEED_VIDEOS)} seeds, {len(todo_seeds)} not "
                f"yet done")
            for vid in todo_seeds:
                ch = check_video(vid, "seed") if vid not in videos_checked else None
                seeds_done.add(vid)
                if ch:
                    channels_found.add(ch)
                flush()
            if complexity >= 1:
                channels = sorted(channels_found - channels_done)
                dbg(f"ring 1: {len(channels)} channels to sweep")
                for ch in channels:
                    if ch in channels_done:
                        continue
                    for vid in channel_video_ids(ch):
                        if vid not in videos_checked:
                            check_video(vid, f"channel {ch}")
                    channels_done.add(ch)
                    flush()
        except BotWall as e:
            flush()
            print(f"ABORT: YouTube bot-wall ({e}); rerun later, cursor is "
                  f"saved", file=sys.stderr)
            dbg(f"crawl ABORT (bot-wall): {e}")
            raise SystemExit(3)

        flush()
        session.log(f"Crawl session {session.session_id} "
                    f"(complexity {complexity}):")
        session.log(f"  seeds done (all time): {len(seeds_done)}/"
                    f"{len(SEED_VIDEOS)}")
        session.log(f"  channels swept (all time): {len(channels_done)}")
        session.log(f"  videos checked (all time): {len(videos_checked)}, "
                    f"verified with stored subtitle: {len(videos_verified)}")
        dbg(f"crawl end: session={session.session_id}, "
            f"checked={len(videos_checked)}, verified={len(videos_verified)}")


SOURCE = YoutubeCaptionsSource()
