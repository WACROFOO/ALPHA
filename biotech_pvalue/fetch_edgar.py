"""Step 1: find and download the event press releases from EDGAR.

- EDGAR full-text search (efts.sec.gov) for 8-K documents mentioning
  "primary endpoint" or "topline", one calendar month at a time.
- Keep EX-99.x exhibits of companies with SIC 2834/2836.
- Per filing, the press release is the lowest-numbered matching EX-99.x.
- Download that exhibit, the filing's SEC header (.hdr.sgml: the official
  acceptance timestamp, in US Eastern time) and the company's submissions JSON
  (current tickers).
  Note: data.sec.gov's acceptanceDateTime is labelled UTC but is shifted by
  4-5 hours for some filings (checked against the filing index pages), so the
  header file is used instead. The JSON value is kept for the comparison.
Everything is cached under data/raw; reruns are offline.

Output: data/derived/filings.csv
"""
import json
import re
import sys
from concurrent.futures import ThreadPoolExecutor
from urllib.parse import urlencode
from zoneinfo import ZoneInfo

import pandas as pd

import config
from common import cached, sec_get

EFTS = "https://efts.sec.gov/LATEST/search-index?"
ET = ZoneInfo("America/New_York")


def month_ranges():
    for start in pd.date_range(config.START_DATE, config.END_DATE, freq="MS"):
        end = start + pd.offsets.MonthEnd(0)
        yield start.strftime("%Y-%m-%d"), end.strftime("%Y-%m-%d")


def search_month(start, end):
    hits, frm = [], 0
    while True:
        params = {"q": config.SEARCH_QUERY, "forms": config.FORMS,
                  "dateRange": "custom", "startdt": start, "enddt": end,
                  "from": frm}
        path = config.RAW / "efts" / f"{start[:7]}_from{frm:05d}.json"
        page = json.loads(cached(path, lambda: sec_get(EFTS + urlencode(params))))
        total = page["hits"]["total"]
        if total.get("relation") != "eq" or total["value"] >= 10_000:
            raise RuntimeError(f"too many hits in {start}: split the window")
        hits += page["hits"]["hits"]
        frm += 100
        if frm >= total["value"] or not page["hits"]["hits"]:
            return hits


def exhibit_rank(file_type):
    m = re.match(r"EX-99(?:\.(\d+))?", file_type or "")
    return int(m.group(1) or 0) if m else None


def candidate_docs():
    rows = []
    for start, end in month_ranges():
        for h in search_month(start, end):
            s = h["_source"]
            rank = exhibit_rank(s.get("file_type"))
            if s.get("form") != "8-K" or rank is None:
                continue
            if not config.SICS & set(s.get("sics") or []):
                continue
            rows.append({"adsh": s["adsh"], "cik": int(s["ciks"][0]),
                         "file_date": s["file_date"],
                         "filename": h["_id"].split(":", 1)[1],
                         "file_type": s["file_type"], "rank": rank,
                         "sics": ",".join(s.get("sics") or []),
                         "items": ",".join(s.get("items") or []),
                         "display_name": (s.get("display_names") or [""])[0]})
        print(f"searched {start[:7]}: {len(rows)} candidate exhibits so far",
              flush=True)
    df = pd.DataFrame(rows).drop_duplicates(["adsh", "filename"])
    # one press release per filing: lowest EX-99 number, then filename
    df = df.sort_values(["adsh", "rank", "filename"]).drop_duplicates("adsh")
    return df.reset_index(drop=True)


def exhibit_path(row):
    return config.RAW / "exhibits" / f"{row.adsh}__{row.filename}"


def fetch_exhibit(row):
    url = (f"https://www.sec.gov/Archives/edgar/data/{row.cik}/"
           f"{row.adsh.replace('-', '')}/{row.filename}")
    return cached(exhibit_path(row), lambda: sec_get(url, ok_404=True)) is not None


def header_path(row):
    return config.RAW / "headers" / f"{row.adsh}.hdr.sgml"


def fetch_header(row):
    url = (f"https://www.sec.gov/Archives/edgar/data/{row.cik}/"
           f"{row.adsh.replace('-', '')}/{row.adsh}.hdr.sgml")
    raw = cached(header_path(row), lambda: sec_get(url)).decode("latin-1")
    acc = re.search(r"<ACCEPTANCE-DATETIME>(\d{14})", raw).group(1)
    items = re.findall(r"<ITEMS>([\d.]+)", raw)
    return pd.Timestamp(acc).tz_localize(ET).isoformat(), ",".join(items)


def submissions(cik):
    """All filings of a company: {adsh: acceptanceDateTime}, tickers, name."""
    base = "https://data.sec.gov/submissions/"
    main = json.loads(cached(config.RAW / "submissions" / f"CIK{cik:010d}.json",
                             lambda: sec_get(f"{base}CIK{cik:010d}.json")))
    blocks = [main["filings"]["recent"]]
    for f in main["filings"].get("files", []):
        blocks.append(json.loads(cached(config.RAW / "submissions" / f["name"],
                                        lambda: sec_get(base + f["name"]))))
    accept = {}
    for b in blocks:
        accept.update(zip(b["accessionNumber"], b["acceptanceDateTime"]))
    return {"accept": accept, "tickers": main.get("tickers") or [],
            "exchanges": main.get("exchanges") or [], "name": main.get("name"),
            "sic": main.get("sic")}


def main():
    df = candidate_docs()
    print(f"{len(df)} filings with an EX-99 hit from SIC {sorted(config.SICS)}")

    with ThreadPoolExecutor(24) as pool:
        df["have_text"] = list(pool.map(fetch_exhibit, df.itertuples()))
        hdr = list(pool.map(fetch_header, df.itertuples()))
        ciks = sorted(df.cik.unique())
        subs = dict(zip(ciks, pool.map(submissions, ciks)))
    print(f"downloaded exhibits; {len(ciks)} companies")

    def accepted_json(row):
        raw = subs[row.cik]["accept"].get(row.adsh)
        return pd.Timestamp(raw).tz_convert(ET).isoformat() if raw else None

    df["accepted_et"] = [a for a, _ in hdr]          # official, from the header
    df["items"] = [i for _, i in hdr]                # official item list
    df["accepted_json_et"] = [accepted_json(r) for r in df.itertuples()]
    diff = (pd.to_datetime(df.accepted_json_et, utc=True)
            - pd.to_datetime(df.accepted_et, utc=True)).dt.total_seconds() / 3600
    print("data.sec.gov acceptanceDateTime minus header time (hours):")
    print(diff.round().value_counts().to_string())
    df["tickers"] = [",".join(subs[c]["tickers"]) for c in df.cik]
    df["exchanges"] = [",".join(x or "" for x in subs[c]["exchanges"]) for c in df.cik]
    df["company"] = [subs[c]["name"] for c in df.cik]
    df["exhibit_path"] = [str(exhibit_path(r).relative_to(config.ROOT))
                          for r in df.itertuples()]
    out = config.DERIVED / "filings.csv"
    out.parent.mkdir(parents=True, exist_ok=True)
    df.to_csv(out, index=False)
    print(f"wrote {out}: {len(df)} rows, "
          f"{df.accepted_et.isna().sum()} without acceptance time, "
          f"{(~df.have_text).sum()} without text, "
          f"{(df.tickers == '').sum()} without a current ticker")


if __name__ == "__main__":
    sys.exit(main())
