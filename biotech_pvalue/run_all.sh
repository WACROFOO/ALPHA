#!/usr/bin/env bash
# Rebuild everything from the raw cache (data/raw). Missing raw files are
# downloaded once and cached; with a full cache this runs offline.
set -euo pipefail
cd "$(dirname "$0")"
python3 fetch_edgar.py           # EDGAR search + exhibits + acceptance times
python3 freeze_classifier.py     # no-op if classifier.py matches the frozen hash
python3 classify_events.py       # blind labels + 30-label spot checks
python3 fetch_prices.py          # Yahoo prices (survivorship-biased, see README)
python3 backtest.py --period dev
python3 variants.py              # dev-only variants, appended to the log
python3 backtest.py --period holdout   # reproduction run after the first
python3 descriptives.py          # post-hoc descriptive numbers (not in verdict)
python3 report.py
