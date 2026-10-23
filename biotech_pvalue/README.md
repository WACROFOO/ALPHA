# Do biotech "positive results" press releases without a p-value underperform?

**Verdict: NO EDGE.** Full numbers: [`results/REPORT.md`](results/REPORT.md).

> **Data caveat (read first).** No free daily price source that includes
> delisted stocks was reachable from the build environment. Prices come from
> Yahoo Finance, which drops delisted symbols, and EDGAR maps companies only to
> their *current* tickers. 49% of 2015–2022 events and 15% of 2023–2025 events
> could not be priced for that reason. The bias check "data includes delisted
> stocks" fails, so this data could never support an "edge" verdict.
> To fix it, add a Tiingo/EODHD/CRSP source in `fetch_prices.py` and rerun
> `run_all.sh`.

## Hypothesis and rules (pre-registered in `config.py` before any return existed)

- **Events:** 8-K press-release exhibits (EX-99.x) from SIC 2834/2836 companies, 2015–2025, mentioning "topline" or "primary endpoint". Found with EDGAR full-text search (13,581 filings, 839 companies). 8-Ks with Item 2.02 (earnings) are excluded.
- **Blind labels:** a rule-based classifier that sees only redacted text, with tickers, dates, dollar amounts and file names removed. It labels claims-success (yes/no/unclear), p-value for the primary endpoint (yes/no) and phase. It was frozen by SHA-256 before any return was computed. There is no LLM key in the environment, so the frozen "prompt" is `classifier.py`.
- **Trade:** among releases that claim success, go long those that give a p-value and short those that don't. Each leg is measured against XBI.
  - Entry is the first open strictly after the official SEC acceptance time.
  - Exits are at 5, 20 and 60 sessions.
  - Costs are 0.5% per round trip, plus a 20% a year borrow fee on shorts.
  - Skip stocks under $1 or under $500k average daily dollar volume.
  - At most one event per company per 30 days.
- **Success:** the net 20-day spread must be ≥ 3% in the 2023–2025 holdout and in most years from 2015 to 2025.

## Results (20-day hold, abnormal return vs XBI)

| | N p-value / no-p | Mean AR p / no-p | Median AR p / no-p | Net spread [95% CI] | Placebo beaten | Trimmed net |
|---|---|---|---|---|---|---|
| Dev 2015–2022 | 90 / 152 | +1.7% / +1.6% | −2.7% / −2.6% | **−2.5%** [−10.6%, +5.2%] | 50% | −3.0% |
| Holdout 2023–2025 (run once) | 74 / 122 | +2.1% / −1.6% | −4.0% / −4.2% | **+1.1%** [−8.3%, +12.1%] | 75% | −3.9% |

Only 2 of 11 years clear 3%: 2018 and 2024. Five of the eight checks fail:
- S1 (holdout ≥ 3%): fail
- S2 (most years): fail
- B3 (confidence interval excludes 0): fail
- B4 (placebo ≥ 95%): fail
- B5 (trimmed result ≥ 3%): fail

B6 (data includes delisted stocks) also fails, by construction (see the caveat above). B1 (holdout run once) and B2 (all variants logged) pass.

**Portfolio view** (20-day holds, 10% of capital per position, XBI-hedged, after costs):

| Long p-value / short no-p-value | CAGR | Vol | Sharpe | Sortino | Max DD | Calmar |
|---|---|---|---|---|---|---|
| Dev 2015–2022 | −2.7% | 6.9% | −0.37 | −0.50 | −29.0% | −0.09 |
| Holdout 2023–2025 | +2.9% | 18.9% | +0.24 | +0.51 | −18.1% | +0.16 |

The positive holdout figures come almost entirely from one trade: Monopar (MNPR), +274% in 20 days in October 2024. Without it, the holdout net spread is −2.6%. Equal-weighting only the open positions (often 1–2 stocks) is in the report too; it has drawdowns past −90%.

## Interesting findings

1. **The market prices p-values in before you can trade.** From the last close before the filing to the first possible entry, p-value releases had already moved +17% on average (median +4.6%) in development. Releases without a p-value moved +5% (median +0.9%). The holdout shows the same: +23% vs +8%. The market does tell the two groups apart, but within hours, not over the following weeks.
2. **Every group fades, but shorting still loses.** The median stock lags XBI by 2.6–4.2% over 20 days after any "positive results" release, with or without a p-value. Means sit near zero because of rare huge winners (Monopar +274%, Filana/Cassava +332%, Axsome +126%). The short no-p-value leg loses money after costs at 20 and 60 days in both periods (−0.5% to −4.5% per trade); only the holdout 5-day leg made money (+1.1%).
3. **"No p-value" mostly means "early or single-arm trial".** Phase 3 makes up 66% of traded p-value releases but only 31% of no-p-value releases, because single-arm and Phase 1 studies rarely report p-values. Restricting to Phase 3 (variant V3) still shows no effect: +0.0% net at 20 days.
4. **No-p-value releases come from smaller, weaker issuers.** In development, 7.1% of them were skipped for trading under $1, against 2.7% of p-value releases. In the holdout it was 14.2% against 8.1%.
5. **Survivorship bias shrinks the closer you get to today.** In 2015–2022, 49% of events can't be priced because the company has since delisted or been acquired. In 2023–2025 it's 15%. Any backtest on current-ticker data quietly drops more of the early sample.
6. **41% of `data.sec.gov` acceptance timestamps are wrong.** The `acceptanceDateTime` field in the submissions API is off by exactly 4 or 5 hours (UTC offset applied twice) for 5,601 of 13,581 filings. The pipeline uses each filing's `.hdr.sgml` header instead, which matches the EDGAR index pages. With the API times, many pre-market filings would have been entered a day late.
7. **Admitted failures don't drift either.** Releases where the trial missed its primary endpoint (65 tradeable, 2015–2022) show a mean 20-day abnormal return of 0.0% after entry (median −0.1%). There is no post-failure drift to short.
8. **Spin is common.** Of 1,030 success-claiming releases (Item 2.02 excluded), 65% give no p-value for the primary endpoint. That share fell to 23–26% in 2021–2022 and was 32–44% in other years.

## Bugs caught during the build (all fixed before the holdout)

- `data.sec.gov` timestamps were off by 4–5 hours for 41% of filings (see finding 6).
- Companies with no ticker were first priced as Yahoo symbol **NAN**, a muni bond fund. A missing value had been turned into the text "nan". This made the dev sample look 2× larger.
- Classifier false positives: disease names ("Primary Dysmenorrhea") and conditional wording. Every round is in [`results/classifier_dev_log.md`](results/classifier_dev_log.md). All three freezes happened before any return existed (`classifier.frozen.json`).

## Files

| File | What it does |
|---|---|
| `config.py` | Pre-registered rules, thresholds and pass/fail definitions |
| `fetch_edgar.py` | EDGAR full-text search → exhibits, SEC headers (acceptance time, items), submissions JSON (tickers) |
| `classifier.py` / `freeze_classifier.py` | Blind classifier and its hash freeze |
| `classify_events.py` | Labels + `results/spot_check_random30.md`, `results/spot_check_tradeable30.md` |
| `fetch_prices.py` | Yahoo daily prices, with split-unadjusted price for the $1 filter |
| `backtest.py` | Event returns, bootstrap CIs, placebo, trimming, per-year, Sharpe/Sortino/Calmar; `--period holdout` is lock-guarded |
| `variants.py` | Every robustness variant (dev only) → `results/variants_log.csv` |
| `descriptives.py` | Post-hoc numbers quoted above (not part of the verdict) |
| `report.py` | `results/REPORT.md` + charts |

## Rerun

```bash
pip install -r requirements.txt
SEC_USER_AGENT="Your Name you@example.com" ./run_all.sh
```

The raw cache (`data/raw/`, about 1.7 GB of exhibits, SEC headers and price JSON) is not in git. The first run rebuilds it, taking about 1 hour at SEC's rate limit. After that, `run_all.sh` runs offline in about 2 minutes and reproduces every result byte-for-byte. The holdout lock (`results/holdout_lock.json`) records the first holdout run. Later runs are reproduction runs and refuse to execute if the code changed.
