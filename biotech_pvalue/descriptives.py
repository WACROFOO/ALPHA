"""Step 7 (post-hoc, descriptive only): numbers quoted in the README findings.

Runs after the holdout. Nothing here feeds the verdict or changes a rule.
Output: results/extra_descriptives.json
"""
import json

import pandas as pd

import config
from backtest import net_spread


def main():
    out = {}
    for per in ("dev", "holdout"):
        t = pd.read_csv(config.RESULTS / per / "trades.csv", dtype={"tickers": str})
        tr = t[t.status == "traded"]
        g = tr.groupby("pvalue_primary")
        status = t.groupby("pvalue_primary").status.value_counts(normalize=True).unstack(fill_value=0)
        rest = tr.drop(tr.ar_20.idxmax())
        top = tr.loc[tr.ar_20.idxmax()]
        gross = (rest.loc[rest.pvalue_primary == "yes", "ar_20"].mean()
                 - rest.loc[rest.pvalue_primary == "no", "ar_20"].mean())
        out[per] = {
            "n_events_after_30d_rule": len(t),
            "n_traded": len(tr),
            "share_no_price_data": round(float((t.status == "no_price_data").mean()), 3),
            "status_share_by_group": status.round(3).to_dict(orient="index"),
            "pre_entry_move_mean": g.pre_entry_move.mean().round(4).to_dict(),
            "pre_entry_move_median": g.pre_entry_move.median().round(4).to_dict(),
            "phase3_share_traded": g.phase.apply(lambda s: s.isin(["3", "2/3"]).mean()).round(3).to_dict(),
            "largest_ar20": f"{top.ticker} {top.ar_20:+.1%} (p-value given: {top.pvalue_primary})",
            "net_spread_20d_without_largest_trade": round(float(net_spread(gross, 20)), 4),
        }
    f = pd.read_csv(config.DERIVED / "filings.csv")
    off = ((pd.to_datetime(f.accepted_json_et, utc=True)
            - pd.to_datetime(f.accepted_et, utc=True)).dt.total_seconds() / 3600).round()
    out["edgar_json_timestamp_error"] = {
        "share_wrong": round(float((off != 0).mean()), 3),
        "hours_off_counts": off.value_counts().to_dict()}
    out["share_filings_without_current_ticker"] = round(float(f.tickers.isna().mean()), 3)
    (config.RESULTS / "extra_descriptives.json").write_text(json.dumps(out, indent=1, default=str))
    print(json.dumps(out, indent=1, default=str))


if __name__ == "__main__":
    main()
