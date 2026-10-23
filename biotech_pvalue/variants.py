"""Step 5: robustness variants and descriptive analyses, DEVELOPMENT PERIOD ONLY.

Every variant run is appended to results/variants_log.csv (with a timestamp),
so nothing tried is hidden. No variant is ever evaluated on the holdout.

Output: results/variants_log.csv, results/dev/descriptive.json,
        results/dev/car_paths.csv
"""
import json

import numpy as np
import pandas as pd

import config
from backtest import (Prices, analyze, build_trades, cooldown, in_period, load_events,
                      position_paths)
from classify_events import check_frozen

PHASE3 = {"3", "2/3"}
PHASE2 = {"2", "1/2"}

# name, description, universe filter, label column
VARIANTS = [
    ("V0_primary", "pre-registered spec: claims_success=yes, p-value for primary "
     "endpoint yes vs no, Item 2.02 filings excluded", True, None, "pvalue_primary"),
    ("V1_keep_item_2.02", "spec-literal universe: keep 8-Ks with Item 2.02 "
     "(earnings releases)", False, None, "pvalue_primary"),
    ("V2_pvalue_any", "group on ANY p-value in the release instead of one tied to "
     "the primary endpoint", True, None, "pvalue_any"),
    ("V3_phase3_only", "only Phase 3 or 2/3 trials (controls for phase mix)", True,
     lambda e: e.phase.isin(PHASE3), "pvalue_primary"),
    ("V4_phase2_only", "only Phase 2 or 1/2 trials (controls for phase mix)", True,
     lambda e: e.phase.isin(PHASE2), "pvalue_primary"),
    ("V5_phase1_only", "only Phase 1 trials", True,
     lambda e: e.phase == "1", "pvalue_primary"),
]


def variant_trades(exclude_items, filt, px):
    ev = load_events(exclude_items=exclude_items)
    ev = in_period(cooldown(ev[ev.claims_success == "yes"]), "dev")
    if filt is not None:
        ev = ev[filt(ev)]
    tr = build_trades(ev, px)
    return tr[tr.status == "traded"], tr


def car_paths(tr, label, px, n=60):
    """Average cumulative abnormal return (gross, before costs) by day since entry."""
    out = {}
    for grp in ("yes", "no"):
        paths = position_paths(tr[tr[label] == grp], n, px, "long")
        # undo the cost haircut so the paths are gross
        rows = []
        for p in paths:
            p = p.copy()
            p.iloc[0] += config.ROUND_TRIP_COST / 2
            p.iloc[-1] += config.ROUND_TRIP_COST / 2
            rows.append((1 + p.reset_index(drop=True)).cumprod() - 1)
        m = pd.concat(rows, axis=1)
        out["p_value" if grp == "yes" else "no_p_value"] = m.mean(axis=1)
    df = pd.DataFrame(out)
    df.index = df.index + 1
    df.index.name = "trading_day"
    return df


def main():
    check_frozen()
    px = Prices()
    log_path = config.RESULTS / "variants_log.csv"
    stamp = pd.Timestamp.utcnow().isoformat()
    rows, desc = [], {}
    start, end = pd.Timestamp(config.START_DATE), pd.Timestamp(config.DEV_END) + pd.Timedelta(days=90)
    for name, text, excl, filt, label in VARIANTS:
        rng = np.random.default_rng(config.SEED)
        traded, _ = variant_trades(excl, filt, px)
        res, _ = analyze(traded, label, px, rng, start, end, with_portfolio=(name == "V0_primary"))
        h = res["horizons"]
        row = {"run_utc": stamp, "variant": name, "description": text, "period": "dev 2015-2022"}
        for n in config.HORIZONS:
            sp = h[n]["spread"]
            row[f"n_p_{n}"], row[f"n_nop_{n}"] = sp.get("n_p"), sp.get("n_nop")
            row[f"net_spread_{n}"] = sp.get("net_spread")
            row[f"net_spread_ci_{n}"] = sp.get("net_spread_ci")
            row[f"median_spread_{n}"] = sp.get("median_spread")
            row[f"placebo_beaten_{n}"] = h[n].get("placebo", {}).get("share_of_shuffles_beaten")
        rows.append(row)
        print(f"{name}: n_p={row['n_p_20']} n_nop={row['n_nop_20']} "
              f"net20={row['net_spread_20']}")
        if name == "V0_primary":
            car_paths(traded, label, px).to_csv(config.RESULTS / "dev" / "car_paths.csv")
            desc["primary_trades"] = traded

    # ---- descriptive analyses (dev only) ---------------------------------
    ev_all = in_period(load_events(exclude_items=True), "dev")
    t0 = desc.pop("primary_trades")
    out = {}
    # 1) what happens after releases that admit failure (claims_success == "no")
    fail = cooldown(ev_all[ev_all.claims_success == "no"])
    ft = build_trades(fail, px)
    ft = ft[ft.status == "traded"]
    out["failed_trials"] = {n: {"n": int(ft[f"ar_{n}"].notna().sum()),
                                "mean_ar": float(ft[f"ar_{n}"].mean()),
                                "median_ar": float(ft[f"ar_{n}"].median())}
                            for n in config.HORIZONS}
    # 2) the move that happened before entry was possible
    out["pre_entry_move"] = {g: {"n": int(x.notna().sum()), "mean": float(x.mean()),
                                 "median": float(x.median())}
                             for g, x in (("p_value", t0.loc[t0.pvalue_primary == "yes", "pre_entry_move"]),
                                          ("no_p_value", t0.loc[t0.pvalue_primary == "no", "pre_entry_move"]))}
    # 3) phase mix by group (traded primary events)
    out["phase_mix"] = pd.crosstab(t0.phase, t0.pvalue_primary).to_dict()
    # 4) where events were lost (survivorship) by group, primary universe
    yes = cooldown(ev_all[ev_all.claims_success == "yes"])
    allt = build_trades(yes, px)
    out["status_by_group"] = pd.crosstab(allt.status, allt.pvalue_primary).to_dict()
    # 5) share of releases giving a p-value, by year (all claims_success=yes, all years)
    every = load_events(exclude_items=True)
    every = every[every.claims_success == "yes"]
    out["pvalue_rate_by_year"] = every.groupby("year").pvalue_primary.apply(
        lambda s: float((s == "yes").mean())).to_dict()
    out["label_counts_all_years"] = {
        "claims_success": every.shape[0],
        "by_pvalue": every.pvalue_primary.value_counts().to_dict()}
    (config.RESULTS / "dev" / "descriptive.json").write_text(json.dumps(out, indent=2, default=str))

    log = pd.DataFrame(rows)
    if log_path.exists():
        log = pd.concat([pd.read_csv(log_path), log], ignore_index=True)
    log.to_csv(log_path, index=False)
    print(json.dumps(out, indent=2, default=str)[:3000])


if __name__ == "__main__":
    main()
