"""Step 4: event returns, statistics, bias checks and portfolio metrics.

    python backtest.py --period dev       # 2015-2022, as often as you like
    python backtest.py --period holdout   # 2023-2025, meant to be run ONCE

Only events inside the requested period are priced, so a dev run never
computes a holdout return.
"""
import argparse
import hashlib
import json

import numpy as np
import pandas as pd

import config
import fetch_prices
from classify_events import check_frozen

ET = "America/New_York"
COST = config.ROUND_TRIP_COST
MIN_PRIOR_BARS = 5  # need some history for the price/ADV filters


# ---------------------------------------------------------------- events ----
def load_events(exclude_items=True):
    f = pd.read_csv(config.DERIVED / "filings.csv", dtype={"tickers": str, "items": str})
    lab = pd.read_csv(config.DERIVED / "labels.csv").drop(columns="exhibit_path")
    ev = f.merge(lab, on="adsh")
    ev = ev[ev.accepted_et.notna()].copy()
    ev["accepted"] = pd.to_datetime(ev.accepted_et, utc=True).dt.tz_convert(ET)
    ev["year"] = ev.accepted.dt.year
    ev["has_excluded_item"] = ev["items"].fillna("").str.split(",").map(
        lambda xs: bool(set(xs) & config.EXCLUDE_ITEMS))
    if exclude_items:
        ev = ev[~ev.has_excluded_item]
    return ev.sort_values("accepted").reset_index(drop=True)


def in_period(ev, period):
    acc = ev.accepted.dt.tz_localize(None)
    if period == "dev":
        return ev[(acc >= config.START_DATE) & (acc < pd.Timestamp(config.DEV_END) + pd.Timedelta(days=1))]
    if period == "holdout":
        return ev[(acc >= config.HOLDOUT_START) & (acc < pd.Timestamp(config.END_DATE) + pd.Timedelta(days=1))]
    if period == "all":
        return ev
    raise ValueError(period)


def cooldown(ev):
    """Keep at most one event per company per COMPANY_COOLDOWN_DAYS (the first)."""
    keep, last = [], {}
    gap = pd.Timedelta(days=config.COMPANY_COOLDOWN_DAYS)
    for r in ev.itertuples():
        t = last.get(r.cik)
        ok = t is None or r.accepted - t >= gap
        keep.append(ok)
        if ok:
            last[r.cik] = r.accepted
    return ev[keep]


# ---------------------------------------------------------------- prices ----
class Prices:
    def __init__(self):
        self.cache = {}
        self.xbi = fetch_prices.load(config.BENCHMARK).set_index("date")
        self.cal = self.xbi.index

    def get(self, ticker):
        if ticker not in self.cache:
            df = fetch_prices.load(ticker)
            self.cache[ticker] = None if df is None else df.set_index("date")
        return self.cache[ticker]

    def for_event(self, tickers, entry_date):
        """First EDGAR ticker whose Yahoo history covers the entry date."""
        if not isinstance(tickers, str):  # NaN: company has no current ticker
            return None, None
        for t in tickers.split(","):
            if not t:
                continue
            s = self.get(fetch_prices.yahoo_symbol(t))
            if s is not None and s.index[0] < entry_date <= s.index[-1] + pd.Timedelta(days=7):
                return t, s
        return None, None

    def entry_session(self, accepted):
        """Index of the first session whose 09:30 ET open is strictly after acceptance."""
        local = accepted.tz_localize(None)
        day = local.normalize()
        i = self.cal.searchsorted(day)
        if i < len(self.cal) and self.cal[i] == day and local < day + pd.Timedelta(hours=9, minutes=30):
            return i
        return self.cal.searchsorted(day, side="right")


def price_event(r, px):
    """One event -> dict with status and abnormal returns for each horizon."""
    out = {"adsh": r.adsh}
    i = px.entry_session(r.accepted)
    if i >= len(px.cal):
        return {**out, "status": "after_price_data"}
    ticker, s = px.for_event(r.tickers, px.cal[i])
    if s is None:
        return {**out, "status": "no_price_data"}
    out["ticker"] = ticker
    j = s.index.searchsorted(px.cal[i])
    last_ok = px.cal[min(i + config.MAX_ENTRY_DELAY, len(px.cal) - 1)]
    if j >= len(s) or s.index[j] > last_ok:
        return {**out, "status": "no_bar_at_entry"}
    entry_date = s.index[j]
    i = px.cal.searchsorted(entry_date)
    prior = s.iloc[max(0, j - config.ADV_WINDOW):j]
    if len(prior) < MIN_PRIOR_BARS:
        return {**out, "status": "short_history"}
    out["prev_raw_close"] = prior.raw_close.iloc[-1]
    out["adv"] = float((prior.close * prior.volume).mean())  # split-invariant
    if out["prev_raw_close"] < config.MIN_PRICE:
        return {**out, "status": "price_below_1"}
    if out["adv"] < config.MIN_ADV_DOLLARS:
        return {**out, "status": "adv_below_500k"}

    bar = s.iloc[j]
    entry_px = bar.open * bar.adjclose / bar.close
    x = px.xbi
    x_entry = x.open.iloc[i] * x.adjclose.iloc[i] / x.close.iloc[i]
    # move that happened before we could trade: last close before acceptance -> entry open
    before = s[s.index < r.accepted.tz_localize(None).normalize()]
    acc_local = r.accepted.tz_localize(None)
    if acc_local.normalize() in s.index and acc_local >= acc_local.normalize() + pd.Timedelta(hours=16):
        before = s[s.index <= acc_local.normalize()]
    if len(before):
        out["pre_entry_move"] = entry_px / before.adjclose.iloc[-1] - 1
    out.update(status="traded", entry_date=entry_date, entry_px=entry_px)
    for n in config.HORIZONS:
        k_cal = i + n - 1
        if k_cal >= len(px.cal):
            out[f"status_{n}"] = "exit_after_price_data"
            continue
        exit_date = px.cal[k_cal]
        k = s.index.searchsorted(exit_date, side="right") - 1
        actual_exit = s.index[k]
        out[f"delisted_{n}"] = bool(s.index[-1] < exit_date)
        out[f"exit_date_{n}"] = actual_exit
        r_s = s.adjclose.iloc[k] / entry_px - 1
        r_x = x.adjclose.loc[actual_exit] / x_entry - 1
        out[f"ret_{n}"] = r_s
        out[f"xbi_{n}"] = r_x
        out[f"ar_{n}"] = r_s - r_x
        out[f"days_{n}"] = int(px.cal.searchsorted(actual_exit) - i + 1)
    return out


def build_trades(ev, px):
    rows = [price_event(r, px) for r in ev.itertuples()]
    return ev.merge(pd.DataFrame(rows), on="adsh")


# ------------------------------------------------------------- statistics ----
def borrow(n):
    return config.BORROW_ANNUAL * n / 252


def net_spread(gross, n):
    return gross - 2 * COST - borrow(n)


def boot(x, stat, rng, n=config.N_BOOT):
    x = np.asarray(x)
    idx = rng.integers(0, len(x), (n, len(x)))
    return stat(x[idx], axis=1)


def group_stats(x, rng):
    x = np.asarray(x, dtype=float)
    if len(x) == 0:
        return {"n": 0}
    m, md = boot(x, np.mean, rng), boot(x, np.median, rng)
    return {"n": int(len(x)), "mean": float(x.mean()), "median": float(np.median(x)),
            "mean_ci": [float(v) for v in np.percentile(m, [2.5, 97.5])],
            "median_ci": [float(v) for v in np.percentile(md, [2.5, 97.5])],
            "hit_rate": float((x > 0).mean())}


def spread_stats(ar_p, ar_n, n, rng):
    ar_p, ar_n = np.asarray(ar_p, float), np.asarray(ar_n, float)
    if len(ar_p) < 2 or len(ar_n) < 2:
        return {"n_p": len(ar_p), "n_nop": len(ar_n)}
    gross = ar_p.mean() - ar_n.mean()
    bs = boot(ar_p, np.mean, rng) - boot(ar_n, np.mean, rng)
    med = np.median(ar_p) - np.median(ar_n)
    bmed = boot(ar_p, np.median, rng) - boot(ar_n, np.median, rng)
    return {"n_p": int(len(ar_p)), "n_nop": int(len(ar_n)),
            "gross_spread": float(gross), "net_spread": float(net_spread(gross, n)),
            "net_spread_ci": [float(net_spread(v, n)) for v in np.percentile(bs, [2.5, 97.5])],
            "median_spread": float(med),
            "median_spread_ci": [float(v) for v in np.percentile(bmed, [2.5, 97.5])],
            "short_nop_net": float(-ar_n.mean() - COST - borrow(n)),
            "long_p_net": float(ar_p.mean() - COST)}


def placebo(ar_p, ar_n, rng, n_shuffles=config.N_PLACEBO):
    pooled = np.concatenate([ar_p, ar_n])
    k = len(ar_p)
    real = np.mean(ar_p) - np.mean(ar_n)
    sh = np.empty(n_shuffles)
    for b in range(n_shuffles):
        perm = rng.permutation(pooled)
        sh[b] = perm[:k].mean() - perm[k:].mean()
    return {"real_gross": float(real), "share_of_shuffles_beaten": float((sh < real).mean()),
            "shuffle_p95": float(np.percentile(sh, 95)), "shuffle_mean": float(sh.mean())}


def trimmed(ar_p, ar_n, n, pct=config.TRIM_PCT):
    pooled = np.concatenate([ar_p, ar_n])
    lo, hi = np.quantile(pooled, [pct, 1 - pct])
    p = ar_p[(ar_p >= lo) & (ar_p <= hi)]
    q = ar_n[(ar_n >= lo) & (ar_n <= hi)]
    gross = p.mean() - q.mean()
    return {"n_p": int(len(p)), "n_nop": int(len(q)), "lo": float(lo), "hi": float(hi),
            "gross_spread": float(gross), "net_spread": float(net_spread(gross, n))}


def per_year(tr, label, n):
    rows = []
    for y, g in tr.groupby("year"):
        p = g.loc[g[label] == "yes", f"ar_{n}"].dropna()
        q = g.loc[g[label] == "no", f"ar_{n}"].dropna()
        row = {"year": int(y), "n_p": len(p), "n_nop": len(q),
               "mean_ar_p": p.mean() if len(p) else np.nan,
               "mean_ar_nop": q.mean() if len(q) else np.nan}
        row["net_spread"] = net_spread(row["mean_ar_p"] - row["mean_ar_nop"], n)
        row["enough_data"] = len(p) >= config.MIN_PER_GROUP_YEAR and len(q) >= config.MIN_PER_GROUP_YEAR
        row["passes"] = bool(row["enough_data"] and row["net_spread"] >= config.MIN_SPREAD)
        rows.append(row)
    return pd.DataFrame(rows)


# ------------------------------------------------- calendar-time portfolio ----
def position_paths(tr, n, px, side):
    """Daily returns of each position (stock minus XBI, after costs)."""
    paths = []
    xr = px.xbi.adjclose
    for r in tr.dropna(subset=[f"ar_{n}"]).itertuples():
        s = px.get(fetch_prices.yahoo_symbol(r.ticker)).adjclose
        dates = px.cal[(px.cal >= r.entry_date) & (px.cal <= getattr(r, f"exit_date_{n}"))]
        sc = s.reindex(dates).ffill()
        xc = xr.reindex(dates)
        x_entry = px.xbi.open.loc[r.entry_date] * px.xbi.adjclose.loc[r.entry_date] / px.xbi.close.loc[r.entry_date]
        rs = sc.pct_change()
        rx = xc.pct_change()
        rs.iloc[0] = sc.iloc[0] / r.entry_px - 1
        rx.iloc[0] = xc.iloc[0] / x_entry - 1
        ar = (rs - rx).fillna(0.0)
        pos = ar if side == "long" else -ar - config.BORROW_ANNUAL / 252
        pos.iloc[0] -= COST / 2
        pos.iloc[-1] -= COST / 2
        paths.append(pos)
    return paths


FIXED_SLOTS = 10  # reporting-only sizing: 10% of sleeve capital per position


def sleeve(paths, cal, fixed=False):
    """Daily sleeve return. Default: equal weight across open positions.
    fixed=True: each position gets 1/FIXED_SLOTS of capital (more open
    positions than slots -> equal weight), idle capital earns 0."""
    if not paths:
        return pd.Series(0.0, index=cal), pd.Series(0, index=cal)
    m = pd.concat(paths, axis=1)
    cnt = m.notna().sum(axis=1).reindex(cal).fillna(0).astype(int)
    if fixed:
        ret = (m.sum(axis=1).reindex(cal).fillna(0.0) / np.maximum(cnt, FIXED_SLOTS))
    else:
        ret = m.mean(axis=1).reindex(cal).fillna(0.0)
    return ret, cnt


def perf(ret, cnt=None):
    ret = ret.astype(float)
    eq = (1 + ret).cumprod()
    years = len(ret) / 252
    cagr = eq.iloc[-1] ** (1 / years) - 1 if years > 0 and eq.iloc[-1] > 0 else -1.0
    vol = ret.std() * np.sqrt(252)
    down = np.sqrt((np.minimum(ret, 0) ** 2).mean()) * np.sqrt(252)
    dd = eq / eq.cummax() - 1
    mdd = dd.min()
    out = {"cagr": float(cagr), "ann_vol": float(vol),
           "sharpe": float(ret.mean() * 252 / vol) if vol > 0 else np.nan,
           "sortino": float(ret.mean() * 252 / down) if down > 0 else np.nan,
           "max_drawdown": float(mdd),
           "calmar": float(cagr / abs(mdd)) if mdd < 0 else np.nan,
           "total_return": float(eq.iloc[-1] - 1),
           "best_day": float(ret.max()), "worst_day": float(ret.min())}
    if cnt is not None:
        out["pct_days_invested"] = float((cnt > 0).mean())
        out["avg_positions_when_invested"] = float(cnt[cnt > 0].mean()) if (cnt > 0).any() else 0.0
    return out


def portfolio(tr, label, n, px, start, end):
    cal = px.cal[(px.cal >= start) & (px.cal <= end)]
    longs = position_paths(tr[tr[label] == "yes"], n, px, "long")
    shorts = position_paths(tr[tr[label] == "no"], n, px, "short")
    rl, cl = sleeve(longs, cal)
    rs, cs = sleeve(shorts, cal)
    combo = 0.5 * rl + 0.5 * rs
    fl, _ = sleeve(longs, cal, fixed=True)
    fs, _ = sleeve(shorts, cal, fixed=True)
    fcombo = 0.5 * fl + 0.5 * fs
    return ({"long_p": perf(rl, cl), "short_nop": perf(rs, cs), "long_short": perf(combo, cl + cs),
             "fixed10_long_p": perf(fl, cl), "fixed10_short_nop": perf(fs, cs),
             "fixed10_long_short": perf(fcombo, cl + cs)},
            pd.DataFrame({"long_p": rl, "short_nop": rs, "long_short": combo,
                          "fixed10_long_p": fl, "fixed10_short_nop": fs,
                          "fixed10_long_short": fcombo}))


# ------------------------------------------------------------------ driver ----
def analyze(tr, label, px, rng, start, end, with_portfolio=True):
    res = {"horizons": {}}
    for n in config.HORIZONS:
        t = tr.dropna(subset=[f"ar_{n}"])
        ar_p = t.loc[t[label] == "yes", f"ar_{n}"].to_numpy()
        ar_n = t.loc[t[label] == "no", f"ar_{n}"].to_numpy()
        h = {"p": group_stats(ar_p, rng), "nop": group_stats(ar_n, rng),
             "spread": spread_stats(ar_p, ar_n, n, rng)}
        if len(ar_p) > 1 and len(ar_n) > 1:
            h["placebo"] = placebo(ar_p, ar_n, rng)
            h["trimmed"] = trimmed(ar_p, ar_n, n)
        res["horizons"][n] = h
    res["per_year"] = per_year(tr, label, config.PRIMARY_HORIZON).to_dict("records")
    curves = None
    if with_portfolio:
        res["portfolio"], curves = {}, {}
        for n in config.HORIZONS:
            res["portfolio"][n], curves[n] = portfolio(tr, label, n, px, start, end)
    return res, curves


def code_hash():
    h = hashlib.sha256()
    for f in ["config.py", "classifier.py", "backtest.py", "fetch_prices.py"]:
        h.update((config.ROOT / f).read_bytes())
    return h.hexdigest()


def primary_trades(period, px):
    allev = load_events(exclude_items=True)
    funnel = {"filings_in_period": len(in_period(allev, period))}
    yes = allev[allev.claims_success == "yes"]
    funnel["claims_success_yes"] = len(in_period(yes, period))
    # 30-day rule runs over all years before the split, so an early-January
    # event still sees the same company's late-December event
    ev = in_period(cooldown(yes), period)
    funnel["after_30d_company_rule"] = len(ev)
    tr = build_trades(ev, px)
    funnel.update({f"status_{k}": int(v) for k, v in tr.status.value_counts().items()})
    return tr, funnel


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--period", choices=["dev", "holdout"], default="dev")
    args = ap.parse_args()
    check_frozen()
    config.RESULTS.mkdir(parents=True, exist_ok=True)
    lock = config.RESULTS / "holdout_lock.json"
    if args.period == "holdout":
        if lock.exists():
            prev = json.loads(lock.read_text())
            print(f"NOTE: holdout was already evaluated at {prev['first_run_utc']}. "
                  "This is a reproduction run; results must match the first run.")
            if prev["code_sha256"] != code_hash():
                raise SystemExit("code changed since the holdout run: refusing to re-run holdout")
        else:
            lock.write_text(json.dumps({"first_run_utc": pd.Timestamp.utcnow().isoformat(),
                                        "code_sha256": code_hash()}, indent=2))

    px = Prices()
    rng = np.random.default_rng(config.SEED)
    tr, funnel = primary_trades(args.period, px)
    if args.period == "dev":
        start, end = pd.Timestamp(config.START_DATE), pd.Timestamp(config.DEV_END) + pd.Timedelta(days=90)
    else:
        start, end = pd.Timestamp(config.HOLDOUT_START), pd.Timestamp(config.END_DATE) + pd.Timedelta(days=90)
    traded = tr[tr.status == "traded"]
    res, curves = analyze(traded, "pvalue_primary", px, rng, start, end)
    res["funnel"] = funnel
    res["period"] = args.period
    res["code_sha256"] = code_hash()
    out = config.RESULTS / args.period
    out.mkdir(parents=True, exist_ok=True)
    (out / "results.json").write_text(json.dumps(res, indent=2, default=str))
    tr.to_csv(out / "trades.csv", index=False)
    for n, c in curves.items():
        c.to_csv(out / f"daily_returns_{n}d.csv")
    print(json.dumps({"funnel": funnel,
                      "net_spread_20d": res["horizons"][20]["spread"]}, indent=2, default=str))


if __name__ == "__main__":
    main()
