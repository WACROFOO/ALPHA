"""Step 3: daily prices from Yahoo Finance (split/dividend data included).

WARNING: Yahoo drops delisted symbols, and EDGAR only lists *current*
tickers, so this price set is survivorship-biased. Events whose company has
no ticker or no Yahoo history are counted as "no price data" and reported.

Raw JSON cached in data/raw/prices/<TICKER>.json.
Output: data/derived/prices/<TICKER>.pkl with columns
  date, open, close, adjclose, volume, raw_close (split-unadjusted)
"""
import json
from concurrent.futures import ThreadPoolExecutor

import numpy as np
import pandas as pd
import requests

import config
from common import RateLimiter, cached, http_get

URL = ("https://query1.finance.yahoo.com/v8/finance/chart/{t}?period1=1388534400"
       "&period2=1792800000&interval=1d&events=div%2Csplit&includeAdjustedClose=true")
_session = requests.Session()
_session.headers.update({"User-Agent": "Mozilla/5.0 (research backtest)"})
_limiter = RateLimiter(4)
OUT = config.DERIVED / "prices"


def raw_json(ticker):
    data = cached(config.RAW / "prices" / f"{ticker}.json",
                  lambda: http_get(URL.format(t=ticker), _session, _limiter, ok_404=True))
    return json.loads(data) if data else None


def parse(js):
    res = (js or {}).get("chart", {}).get("result")
    if not res or not res[0].get("timestamp"):
        return None
    r = res[0]
    q = r["indicators"]["quote"][0]
    tz = r["meta"].get("exchangeTimezoneName", "America/New_York")
    df = pd.DataFrame({
        "date": pd.to_datetime(r["timestamp"], unit="s", utc=True).tz_convert(tz)
                  .tz_localize(None).normalize(),
        "open": q["open"], "close": q["close"], "volume": q["volume"],
        "adjclose": r["indicators"]["adjclose"][0]["adjclose"],
    }).dropna(subset=["open", "close", "adjclose"])
    df = df[(df.close > 0) & (df.open > 0)].drop_duplicates("date", keep="last")
    # Yahoo prices are split-adjusted. Undo splits that happen after each day
    # to get the price actually quoted that day (needed for the $1 filter).
    factor = np.ones(len(df))
    for s in (r.get("events", {}).get("splits") or {}).values():
        sdate = pd.Timestamp(s["date"], unit="s", tz="UTC").tz_convert(tz).tz_localize(None)
        ratio = s["numerator"] / s["denominator"]  # new shares per old share
        factor[(df.date < sdate.normalize()).to_numpy()] *= ratio
    df["raw_close"] = df.close * factor
    return df.reset_index(drop=True)


def load(ticker):
    path = OUT / f"{ticker}.pkl"
    if path.exists():
        return pd.read_pickle(path)
    df = parse(raw_json(ticker))
    if df is not None:
        OUT.mkdir(parents=True, exist_ok=True)
        df.to_pickle(path)
    return df


def yahoo_symbol(t):
    return t.replace(".", "-").upper()


def main():
    f = pd.read_csv(config.DERIVED / "filings.csv", dtype={"tickers": str})
    tickers = {config.BENCHMARK}
    for ts in f.tickers.dropna():
        tickers.update(yahoo_symbol(t) for t in ts.split(",") if t)
    print(f"fetching {len(tickers)} symbols")
    with ThreadPoolExecutor(4) as pool:
        got = dict(zip(sorted(tickers), pool.map(load, sorted(tickers))))
    ok = sum(v is not None for v in got.values())
    print(f"{ok}/{len(tickers)} symbols have Yahoo history")


if __name__ == "__main__":
    main()
