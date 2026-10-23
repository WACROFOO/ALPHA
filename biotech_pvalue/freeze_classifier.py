"""Freeze classifier.py: record its SHA-256 before any return is computed.

Run once. Re-running with a different classifier.py is refused unless
--refreeze is given, and every refreeze is appended to the history so it
shows up in the report.
"""
import datetime as dt
import json
import sys

import config
from classify_events import FROZEN, classifier_hash

returns_exist = (config.DERIVED / "event_returns.csv").exists()
h = classifier_hash()
state = json.loads(FROZEN.read_text()) if FROZEN.exists() else {"history": []}
if state.get("sha256") == h:
    print("already frozen with this hash:", h)
    sys.exit()
if state.get("sha256") and "--refreeze" not in sys.argv:
    sys.exit("a different classifier is already frozen; use --refreeze (logged)")
state["history"].append({"sha256": h,
                         "frozen_at_utc": dt.datetime.now(dt.UTC).isoformat(),
                         "returns_existed_at_freeze": returns_exist})
state["sha256"] = h
FROZEN.write_text(json.dumps(state, indent=2))
print("frozen:", h, "| returns existed:", returns_exist)
