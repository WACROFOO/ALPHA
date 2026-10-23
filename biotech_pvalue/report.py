"""Step 6: assemble results/REPORT.md and charts from the saved results.

Applies the pre-registered success criteria and bias-check rules in
config.py mechanically; nothing here is tuned.
"""
import json

import matplotlib
import numpy as np
import pandas as pd

import config

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

R = config.RESULTS
N0 = config.PRIMARY_HORIZON
INK, INK2, GRID, SURFACE = "#0b0b0b", "#52514e", "#e4e3df", "#fcfcfb"
BLUE, ORANGE, AQUA = "#2a78d6", "#eb6834", "#1baf7a"


def pct(x, d=1):
    return "n/a" if x is None or (isinstance(x, float) and np.isnan(x)) else f"{100 * x:.{d}f}%"


def ci(c):
    return "n/a" if not c else f"[{pct(c[0])}, {pct(c[1])}]"


def load(period):
    p = R / period / "results.json"
    return json.loads(p.read_text()) if p.exists() else None


# ------------------------------------------------------------------ charts ----
def style(ax, title, ylabel):
    ax.set_facecolor(SURFACE)
    ax.set_title(title, loc="left", color=INK, fontsize=11)
    ax.set_ylabel(ylabel, color=INK2, fontsize=9)
    ax.tick_params(colors=INK2, labelsize=8)
    ax.grid(axis="y", color=GRID, linewidth=0.8)
    for s in ("top", "right"):
        ax.spines[s].set_visible(False)
    for s in ("left", "bottom"):
        ax.spines[s].set_color(GRID)


def chart_equity(period):
    f = R / period / f"daily_returns_{N0}d.csv"
    if not f.exists():
        return None
    d = pd.read_csv(f, index_col=0, parse_dates=True)
    fig, ax = plt.subplots(figsize=(8, 3.6), facecolor=SURFACE)
    for col, color, name in (("fixed10_long_p", BLUE, "Long: p-value given"),
                             ("fixed10_short_nop", ORANGE, "Short: no p-value"),
                             ("fixed10_long_short", AQUA, "Long/short 50/50")):
        eq = (1 + d[col]).cumprod()
        ax.plot(eq.index, eq.values, color=color, linewidth=2, label=name)
        ax.annotate(name, (eq.index[-1], eq.values[-1]), xytext=(4, 0),
                    textcoords="offset points", color=INK2, fontsize=8, va="center")
    style(ax, f"Equity, {N0}-day holds, 10% per position, after costs ({period})", "growth of $1")
    ax.legend(frameon=False, fontsize=8, loc="upper left", labelcolor=INK2)
    fig.tight_layout()
    out = R / f"equity_{period}.png"
    fig.savefig(out, dpi=130)
    plt.close(fig)
    return out.name


def chart_car():
    f = R / "dev" / "car_paths.csv"
    if not f.exists():
        return None
    d = pd.read_csv(f, index_col=0)
    fig, ax = plt.subplots(figsize=(8, 3.6), facecolor=SURFACE)
    for col, color, name in (("p_value", BLUE, "p-value given"),
                             ("no_p_value", ORANGE, "no p-value")):
        ax.plot(d.index, 100 * d[col], color=color, linewidth=2, label=name)
        ax.annotate(name, (d.index[-1], 100 * d[col].iloc[-1]), xytext=(4, 0),
                    textcoords="offset points", color=INK2, fontsize=8, va="center")
    ax.axhline(0, color=INK2, linewidth=0.8)
    style(ax, "Mean cumulative return vs XBI after entry, before costs (2015-2022)",
          "cumulative abnormal return, %")
    ax.set_xlabel("trading days after entry", color=INK2, fontsize=9)
    ax.legend(frameon=False, fontsize=8, loc="lower left", labelcolor=INK2)
    fig.tight_layout()
    out = R / "car_paths_dev.png"
    fig.savefig(out, dpi=130)
    plt.close(fig)
    return out.name


def chart_years(years):
    if years.empty:
        return None
    fig, ax = plt.subplots(figsize=(8, 3.4), facecolor=SURFACE)
    y = years.dropna(subset=["net_spread"])
    ax.bar(y.year.astype(str), 100 * y.net_spread, color=BLUE, width=0.6)
    ax.axhline(100 * config.MIN_SPREAD, color=INK2, linestyle="--", linewidth=1)
    ax.annotate("3% bar", (len(y) - 0.5, 100 * config.MIN_SPREAD), xytext=(0, 3),
                textcoords="offset points", color=INK2, fontsize=8, ha="right")
    ax.axhline(0, color=INK2, linewidth=0.8)
    style(ax, f"Net {N0}-day spread by year (long p-value, short no-p-value)", "net spread, %")
    fig.tight_layout()
    out = R / "spread_by_year.png"
    fig.savefig(out, dpi=130)
    plt.close(fig)
    return out.name


# ------------------------------------------------------------------ tables ----
def horizon_table(res):
    rows = ["| Horizon | Group | N | Mean AR | 95% CI (mean) | Median AR | 95% CI (median) | Hit rate |",
            "|---|---|---|---|---|---|---|---|"]
    for n in config.HORIZONS:
        h = res["horizons"][str(n)]
        for key, name in (("p", "p-value given"), ("nop", "no p-value")):
            g = h[key]
            if not g.get("n"):
                rows.append(f"| {n}d | {name} | 0 | | | | | |")
                continue
            rows.append(f"| {n}d | {name} | {g['n']} | {pct(g['mean'])} | {ci(g['mean_ci'])} | "
                        f"{pct(g['median'])} | {ci(g['median_ci'])} | {pct(g['hit_rate'], 0)} |")
    rows += ["", "| Horizon | Gross spread (mean p − mean no-p) | Net spread after costs | 95% CI (net) | "
             "Median spread | 95% CI (median) | Short no-p leg, net | Long p leg, net |",
             "|---|---|---|---|---|---|---|---|"]
    for n in config.HORIZONS:
        s = res["horizons"][str(n)]["spread"]
        if "gross_spread" not in s:
            continue
        rows.append(f"| {n}d | {pct(s['gross_spread'])} | **{pct(s['net_spread'])}** | {ci(s['net_spread_ci'])} | "
                    f"{pct(s['median_spread'])} | {ci(s['median_spread_ci'])} | {pct(s['short_nop_net'])} | "
                    f"{pct(s['long_p_net'])} |")
    return "\n".join(rows)


def portfolio_table(res):
    if "portfolio" not in res:
        return ""
    rows = ["| Hold | Sleeve | CAGR | Ann. vol | Sharpe | Sortino | Max DD | Calmar | Total return | "
            "Days invested | Avg positions |", "|---|---|---|---|---|---|---|---|---|---|---|"]
    names = {"long_p": "Long p-value", "short_nop": "Short no-p-value", "long_short": "Long/short",
             "fixed10_long_p": "Long p-value (10%/pos)", "fixed10_short_nop": "Short no-p (10%/pos)",
             "fixed10_long_short": "Long/short (10%/pos)"}
    for n in config.HORIZONS:
        for k, nm in names.items():
            m = res["portfolio"][str(n)][k]
            rows.append(f"| {n}d | {nm} | {pct(m['cagr'])} | {pct(m['ann_vol'])} | {m['sharpe']:.2f} | "
                        f"{m['sortino']:.2f} | {pct(m['max_drawdown'])} | {m['calmar']:.2f} | "
                        f"{pct(m['total_return'])} | {pct(m.get('pct_days_invested'), 0)} | "
                        f"{m.get('avg_positions_when_invested', 0):.1f} |")
    return "\n".join(rows)


def year_table(years):
    rows = ["| Year | N p-value | N no-p | Mean AR p-value | Mean AR no-p | Net spread | ≥5 per group | ≥3% |",
            "|---|---|---|---|---|---|---|---|"]
    for r in years.itertuples():
        rows.append(f"| {r.year} | {r.n_p} | {r.n_nop} | {pct(r.mean_ar_p)} | {pct(r.mean_ar_nop)} | "
                    f"{pct(r.net_spread)} | {'yes' if r.enough_data else 'no'} | "
                    f"{'PASS' if r.passes else 'fail'} |")
    return "\n".join(rows)


def delisted_table():
    rows = []
    for period in ("dev", "holdout"):
        f = R / period / "trades.csv"
        if not f.exists():
            continue
        t = pd.read_csv(f)
        for n in config.HORIZONS:
            col = f"delisted_{n}"
            if col in t:
                for r in t[t[col] == True].itertuples():  # noqa: E712
                    rows.append(f"| {period} | {r.adsh} | {r.ticker} | {n}d | "
                                f"{getattr(r, f'exit_date_{n}')} | {pct(getattr(r, f'ar_{n}'))} |")
    if not rows:
        return "No traded event lost its price series before exit."
    return "\n".join(["| Period | Accession | Ticker | Horizon | Last traded day used | AR |",
                      "|---|---|---|---|---|---|"] + rows)


def funnel_lines(res):
    return "\n".join(f"- {k.replace('_', ' ')}: {v}" for k, v in res["funnel"].items())


# ------------------------------------------------------------------ verdict ----
def evaluate(dev, hold, years):
    checks, crit = {}, {}
    h = hold["horizons"][str(N0)] if hold else None
    lock = R / "holdout_lock.json"
    if hold:
        first = json.loads(lock.read_text())
        checks["B1 Holdout run exactly once, after development"] = (
            first["code_sha256"] == hold["code_sha256"],
            f"first run {first['first_run_utc']}; code hash unchanged since")
    log = pd.read_csv(R / "variants_log.csv") if (R / "variants_log.csv").exists() else pd.DataFrame()
    checks["B2 Every variant logged; none tuned on holdout"] = (
        bool(len(log)) and set(log.period) == {"dev 2015-2022"},
        f"{log.variant.nunique() if len(log) else 0} variants, {len(log)} logged runs, all on 2015-2022")
    if h:
        s = h["spread"]
        checks["B3 95% bootstrap CI of holdout net spread excludes 0"] = (
            s["net_spread_ci"][0] > 0, f"net {pct(s['net_spread'])}, CI {ci(s['net_spread_ci'])}")
        pl = h.get("placebo", {})
        checks["B4 Placebo: beats ≥95% of 1,000 label shuffles"] = (
            pl.get("share_of_shuffles_beaten", 0) >= 0.95,
            f"beats {pct(pl.get('share_of_shuffles_beaten'), 1)} of shuffles")
        tr = h.get("trimmed", {})
        checks["B5 Still ≥3% net after dropping top/bottom 5%"] = (
            tr.get("net_spread", -1) >= config.MIN_SPREAD, f"trimmed net {pct(tr.get('net_spread'))}")
    checks["B6 Price data includes delisted stocks"] = (
        False, "Yahoo Finance only; delisted/acquired names are missing (see Data caveat)")
    if h:
        crit["S1 Holdout net 20d spread ≥ 3%"] = (h["spread"].get("net_spread", -1) >= config.MIN_SPREAD,
                                                   pct(h["spread"].get("net_spread")))
    passing = int(years.passes.sum()) if len(years) else 0
    n_years = 2025 - 2015 + 1
    crit["S2 Net 20d spread ≥ 3% in most years (2015-2025)"] = (
        passing > n_years / 2, f"{passing} of {n_years} years pass "
        f"({int(years.enough_data.sum()) if len(years) else 0} years have ≥5 events per group)")
    if not h or min(h["spread"].get("n_p", 0), h["spread"].get("n_nop", 0)) < config.MIN_HOLDOUT_PER_GROUP:
        verdict = "not enough data"
    elif all(v[0] for v in crit.values()) and all(v[0] for v in checks.values()):
        verdict = "edge"
    else:
        verdict = "no edge"
    return checks, crit, verdict


def main():
    dev, hold = load("dev"), load("holdout")
    years = pd.DataFrame((dev or {}).get("per_year", []) + (hold or {}).get("per_year", []))
    checks, crit, verdict = evaluate(dev, hold, years)
    charts = [c for c in (chart_equity("dev"), chart_equity("holdout"), chart_car(),
                          chart_years(years)) if c]
    frozen = json.loads((config.ROOT / "classifier.frozen.json").read_text())
    desc_f = R / "dev" / "descriptive.json"
    desc = json.loads(desc_f.read_text()) if desc_f.exists() else {}
    log = pd.read_csv(R / "variants_log.csv") if (R / "variants_log.csv").exists() else pd.DataFrame()

    L = [f"# Biotech p-value backtest: report", "",
         f"**Verdict: {verdict.upper()}**", "",
         "Hypothesis: biotech press releases that claim positive trial results but give no p-value for "
         "the primary endpoint underperform those that give one, over 5-60 trading days.", "",
         "## Success criteria (pre-registered in config.py)", "",
         "| Criterion | Result | Pass? |", "|---|---|---|"]
    L += [f"| {k} | {v[1]} | {'PASS' if v[0] else 'FAIL'} |" for k, v in crit.items()]
    L += ["", "## Bias checks", "", "| Check | Detail | Pass? |", "|---|---|---|"]
    L += [f"| {k} | {v[1]} | {'PASS' if v[0] else 'FAIL'} |" for k, v in checks.items()]
    L += ["", "## Data caveat: survivorship bias", "",
          "No source of daily prices that includes delisted stocks was reachable from this environment "
          "(no Tiingo/EODHD/Polygon/CRSP key; Stooq blocked). Prices come from Yahoo Finance, which drops "
          "delisted symbols, and EDGAR only maps companies to their *current* tickers. Every company that "
          "later went bankrupt, was acquired, or was delisted is missing, so check B6 fails by construction "
          "and the verdict can never be \"edge\" on this data.", ""]
    for period, res in (("Development 2015-2022", dev), ("Holdout 2023-2025 (single run)", hold)):
        if not res:
            continue
        L += [f"## {period}", "", "Event funnel:", "", funnel_lines(res), "",
              "AR = stock return minus XBI return over the same window (entry at the first open after the "
              "8-K acceptance time, exit at the close of trading day N). Net spread = mean AR(p) − mean "
              "AR(no-p) − 2 × 0.5% − 20% × N/252 borrow.", "", horizon_table(res), "",
              "Placebo and trimming:", "",
              "| Horizon | Real gross spread | Shuffles beaten | 95th pct of shuffles | Trimmed net spread (N p / N no-p) |",
              "|---|---|---|---|---|"]
        for n in config.HORIZONS:
            h = res["horizons"][str(n)]
            if "placebo" in h:
                L.append(f"| {n}d | {pct(h['placebo']['real_gross'])} | "
                         f"{pct(h['placebo']['share_of_shuffles_beaten'], 1)} | "
                         f"{pct(h['placebo']['shuffle_p95'])} | {pct(h['trimmed']['net_spread'])} "
                         f"({h['trimmed']['n_p']} / {h['trimmed']['n_nop']}) |")
        L += ["", "Calendar-time portfolio (daily XBI hedge, costs and borrow included, rf = 0). Two sizings: "
              "equal weight across whatever positions are open (often 1-3 names, so very concentrated), and "
              "10% of capital per position with idle cash at 0. The 10% sizing was added after seeing the "
              "development-period drawdowns; it is reporting only and does not enter the verdict.", "",
              portfolio_table(res), ""]
    L += ["## Results by year (20-day hold)", "", year_table(years), ""]
    L += ["## Stocks that stopped trading before exit (last traded price used)", "", delisted_table(), ""]
    if len(log):
        L += ["## Every variant tried (development period only)", "",
              "| Run (UTC) | Variant | Description | N p / no-p (20d) | Net spread 20d | 95% CI | Shuffles beaten |",
              "|---|---|---|---|---|---|---|"]
        for r in log.itertuples():
            cil = json.loads(r.net_spread_ci_20) if isinstance(r.net_spread_ci_20, str) else None
            L.append(f"| {r.run_utc[:19]} | {r.variant} | {r.description} | {r.n_p_20} / {r.n_nop_20} | "
                     f"{pct(r.net_spread_20)} | {ci(cil)} | {pct(r.placebo_beaten_20, 1)} |")
        L.append("")
    if desc:
        L += ["## Descriptive numbers (development period)", "", "```json",
              json.dumps(desc, indent=1)[:6000], "```", ""]
    L += ["## Classifier", "",
          f"Rule-based, blind (sees only redacted text), frozen with SHA-256 `{frozen['sha256'][:16]}…`; "
          f"freeze history: {len(frozen['history'])} entr{'y' if len(frozen['history']) == 1 else 'ies'}, "
          f"returns existed at any freeze: {any(h['returns_existed_at_freeze'] for h in frozen['history'])}. "
          "Spot checks: `spot_check_random30.md`, `spot_check_tradeable30.md`.", ""]
    L += ["## Charts", ""] + [f"![{c}]({c})" for c in charts]
    (R / "REPORT.md").write_text("\n".join(L) + "\n")
    print(f"verdict: {verdict}")


if __name__ == "__main__":
    main()
