# Classifier development log (text only, before any return was computed)

All tuning below used press-release text only. No prices or returns existed
when these rounds ran. They are listed because "every variant tried" includes
the classifier.

| Round | Change | Why (what the text samples showed) |
|---|---|---|
| 1 | First rule set: success = positive/negative phrases anywhere in the first 3,000 chars; p-value = p-value in a sentence mentioning the primary endpoint or the next sentence. | Baseline. |
| 2 | Success claims count only in the headline area (first 1,000 chars) or in a sentence containing "today"; recaps ("previously", "In March…", "announced…" without "today", "based on") are ignored. Bullets split sentences. "HER2-positive"-style compounds no longer match "positive … data". Dropped "positive outcome". | Earnings and business-update releases recap older trial news ("Announced positive topline data from…"); those were labelled as new success claims. |
| 3 | Failure phrases are ignored when hedged ("if", "may", "risk", …). | Hypothetical failure language. |
| 4 | Added `pvalue_any` (any p-value in the body) as an exploratory label only. | Some releases give p-values without saying which endpoint is primary; strict label kept as the primary definition. |
| 5 | "Futility" counts as failure only as "stopped for/due to futility" or "futility boundary met". | "Positive interim futility analysis" was labelled a failure. |
| 6 | Redaction: exchange tags like "NasdaqGS: XXXX", document file names (which embed tickers, e.g. `abcd-ex99_1.htm`), and upper-case dates. | Ticker and date leaks into the classifier input. |
| — | **Freeze 1** (`4a4d8e83…`), all 13,581 releases labelled, 30-label spot checks reviewed. | |
| 7 | "primary" must be followed by an endpoint word ("primary endpoint/objective/outcome/goal/analysis"); "following …", "assuming …", "upon …" mark recaps/conditionals. | Spot check: "Primary Dysmenorrhea" (a disease name) read as a met primary endpoint; "available following positive Phase 3 data" (a loan condition) read as a result. |
| — | **Freeze 2** (`c2f3f0d2…`). Spot check redrawn from the trading universe (no Item 2.02). | |
| 8 | Headline "… Quarter/Full-Year … (Financial) Results" marks an earnings release → claims_success = unclear. Words between "positive" and "data/results" restricted to a fixed descriptor list (topline, interim, Phase 2b, 52-week, …). | Spot check: an earnings release filed without Item 2.02 (Alnylam), "positive in-class competitor data", "Positive Recommendation by Independent Data Monitoring Committee". |
| — | **Freeze 3, final** (`4aacf43c…`). No change after this point. Final tradeable spot check: 27/30 are genuine new trial-result success claims; 3 are regulatory/business news citing earlier positive data. | |

Primary universe decision (made with the rounds above, before returns): drop 8-Ks
with Item 2.02 (quarterly results); keep them as logged variant V1.
