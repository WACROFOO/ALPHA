"""Step 2: label every press release with the frozen, blind classifier.

Only the redacted exhibit text is passed to classifier.classify(); the
ticker, CIK, dates and prices never reach it.

Outputs: data/derived/labels.csv
         results/spot_check_random30.md        (30 random releases)
         results/spot_check_tradeable30.md     (30 random 'claims success')
"""
import hashlib
import json
import re
import sys
from concurrent.futures import ProcessPoolExecutor

import warnings

import pandas as pd
from bs4 import BeautifulSoup, XMLParsedAsHTMLWarning

import classifier
import config

FROZEN = config.ROOT / "classifier.frozen.json"
warnings.filterwarnings("ignore", category=XMLParsedAsHTMLWarning)


def classifier_hash():
    return hashlib.sha256((config.ROOT / "classifier.py").read_bytes()).hexdigest()


def check_frozen():
    if not FROZEN.exists():
        sys.exit("classifier not frozen: run freeze_classifier.py first")
    if json.loads(FROZEN.read_text())["sha256"] != classifier_hash():
        sys.exit("classifier.py changed after it was frozen; refusing to run")


def exhibit_text(path):
    raw = (config.ROOT / path).read_bytes()
    try:
        html = raw.decode("utf-8")
    except UnicodeDecodeError:
        html = raw.decode("latin-1")
    # drop embedded (uuencoded / base64) blobs before parsing
    html = re.sub(r"(?is)<(script|style)[^>]*>.*?</\1>", " ", html)
    if path.lower().endswith(".txt") and "<html" not in html[:2000].lower():
        return html
    soup = BeautifulSoup(html, "lxml")
    # block-level tags end a sentence-ish chunk; keep that boundary
    for tag in soup.find_all(["p", "div", "br", "tr", "li", "h1", "h2", "h3", "h4", "table"]):
        tag.insert_after(" \n ")
    return soup.get_text(" ")


def label_one(path):
    redacted = classifier.redact(exhibit_text(path))
    out = classifier.classify(redacted)
    out["n_chars"] = len(redacted)
    return out


def snippet(redacted, n=1200):
    return redacted[:n] + (" ..." if len(redacted) > n else "")


def evidence(redacted):
    """Sentences that drove the labels, for human spot checks."""
    text = classifier.body(redacted)
    pos, neg = classifier.success_evidence(text[:classifier.LEAD_CHARS])
    return pos[:3], neg[:3], classifier.pvalue_evidence(text)[:3]


def write_spot_check(df, path, title, seed):
    sample = df.sample(min(30, len(df)), random_state=seed)
    lines = [f"# {title}", "",
             "Each entry: labels from the frozen classifier, the opening of the redacted",
             "text it saw, and the sentences that triggered each label.", ""]
    for k, r in enumerate(sample.itertuples(), 1):
        red = classifier.redact(exhibit_text(r.exhibit_path))
        pos, neg, pv = evidence(red)
        lines += [f"## {k}. accession {r.adsh}",
                  f"**claims_success = {r.claims_success} | pvalue_primary = "
                  f"{r.pvalue_primary} | phase = {r.phase}**", "",
                  "> " + snippet(red).replace("\n", " "), ""]
        for name, ss in (("success-positive", pos), ("success-negative", neg),
                         ("p-value", pv)):
            for s in ss:
                lines.append(f"- *{name}*: {s[:400]}")
        lines.append("")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines))


def main():
    check_frozen()
    f = pd.read_csv(config.DERIVED / "filings.csv")
    f = f[f.have_text].copy()
    with ProcessPoolExecutor() as pool:
        labels = list(pool.map(label_one, f.exhibit_path, chunksize=50))
    lab = pd.concat([f[["adsh", "exhibit_path"]].reset_index(drop=True),
                     pd.DataFrame(labels)], axis=1)
    lab.to_csv(config.DERIVED / "labels.csv", index=False)
    print(lab.claims_success.value_counts().to_string())
    print(pd.crosstab(lab.claims_success, lab.pvalue_primary).to_string())
    write_spot_check(lab, config.RESULTS / "spot_check_random30.md",
                     "Spot check: 30 random press releases (all labels)", config.SEED)
    items = f.set_index("adsh")["items"].fillna("").str.split(",")
    tradeable = lab[(lab.claims_success == "yes")
                    & ~lab.adsh.map(lambda a: bool(set(items[a]) & config.EXCLUDE_ITEMS))]
    write_spot_check(tradeable, config.RESULTS / "spot_check_tradeable30.md",
                     "Spot check: 30 random releases in the trading universe "
                     "(claims_success = yes, no Item 2.02)", config.SEED + 1)


if __name__ == "__main__":
    main()
