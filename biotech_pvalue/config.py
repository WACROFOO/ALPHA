"""Pre-registered parameters. Written before any return was computed.

Nothing in this file may be changed after looking at results. If a rule has
to change, add a new variant to variants_log.csv instead and report it.
"""
from pathlib import Path
import os

ROOT = Path(__file__).resolve().parent
RAW = ROOT / "data" / "raw"          # untouched downloads (cache)
DERIVED = ROOT / "data" / "derived"  # everything rebuilt from RAW
RESULTS = ROOT / "results"

# SEC asks for a descriptive User-Agent with a contact address.
SEC_USER_AGENT = os.environ.get(
    "SEC_USER_AGENT",
    "biotech-pvalue-backtest research-script admin@biotech-pvalue-research.org",
)
SEC_MAX_RPS = 8  # SEC limit is 10 requests/second

# ---- Event universe -------------------------------------------------------
SEARCH_QUERY = '"primary endpoint" OR "topline"'
FORMS = "8-K"
SICS = {"2834", "2836"}
START_DATE = "2015-01-01"
END_DATE = "2025-12-31"
# Press release = lowest-numbered EX-99.x exhibit of the filing that matched.
# Primary universe drops 8-Ks that include Item 2.02 (quarterly results):
# those releases mostly recap trial news that was announced earlier, so they
# are not trial-result events. The spec-literal universe (keeping them) is run
# as a logged robustness variant on the development period only.
EXCLUDE_ITEMS = {"2.02"}

# ---- Sample split ---------------------------------------------------------
DEV_END = "2022-12-31"      # develop on 2015-2022
HOLDOUT_START = "2023-01-01"  # 2023-2025, run exactly once

# ---- Trading rules --------------------------------------------------------
HORIZONS = [5, 20, 60]      # trading days held; enter at open of day 1,
                            # exit at close of day N (N sessions in total)
PRIMARY_HORIZON = 20
BENCHMARK = "XBI"
MIN_PRICE = 1.00            # raw (split-unadjusted) close the day before entry
MIN_ADV_DOLLARS = 500_000   # mean close*volume over the 20 sessions before entry
ADV_WINDOW = 20
MAX_ENTRY_DELAY = 3         # if the stock has no bar on entry day, use its next
                            # bar within this many sessions, otherwise skip
ROUND_TRIP_COST = 0.005     # 0.5% per round trip, every trade
BORROW_ANNUAL = 0.20        # shorts only, charged per trading day held / 252
COMPANY_COOLDOWN_DAYS = 30  # max one event per company per 30 calendar days.
                            # Applied to claims_success == "yes" releases
                            # (the tradeable set) in time order, keeping the
                            # first, before price filters and independent of
                            # the p-value label.

# ---- Hypothesis and success criteria (decided before results) -------------
# Universe for the test: releases labelled claims_success == "yes".
# Group A ("nop"): no p-value for the primary endpoint  -> short
# Group B ("p"):   p-value given for the primary endpoint -> long
# Abnormal return AR = stock return - XBI return over the same window.
# Net spread(N) = [mean(AR_p) - cost] + [-mean(AR_nop) - cost - borrow(N)]
#              = mean(AR_p) - mean(AR_nop) - 2*0.5% - 20%*N/252
# Success requires ALL of:
#   S1 net spread(20) >= 3% in the 2023-2025 holdout
#   S2 net spread(20) >= 3% in more than half of the calendar years 2015-2025
#      that have >= MIN_PER_GROUP_YEAR events in each group (years with fewer
#      events count as "not passing")
MIN_SPREAD = 0.03
MIN_PER_GROUP_YEAR = 5
# Verdict: "not enough data" if the holdout has < MIN_HOLDOUT_PER_GROUP
# events in either group at 20d; "edge" only if S1, S2 and every bias check
# pass; otherwise "no edge".
MIN_HOLDOUT_PER_GROUP = 30

# ---- Bias-check pass rules (holdout, 20d) ---------------------------------
#  B1 holdout touched exactly once                    -> pass/fail
#  B2 all variants logged, none chosen on holdout      -> pass/fail
#  B3 95% bootstrap CI of the net spread excludes 0    -> pass/fail
#  B4 real gross spread beats >= 95% of 1,000 label shuffles (one-sided)
#  B5 net spread still >= 3% after dropping the pooled top/bottom 5% of AR
#  B6 price data includes delisted stocks              -> pass/fail
N_BOOT = 10_000
N_PLACEBO = 1_000
TRIM_PCT = 0.05
SEED = 20150101
