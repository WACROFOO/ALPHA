"""Blind rule-based press-release classifier.

The classifier is a pure function of press-release text. Before it sees the
text, redact() removes exchange/ticker tags, dates and dollar amounts, and the
calling script passes nothing else (no ticker, CIK, date or price).

Labels
  claims_success: "yes" / "no" / "unclear"  (judged on the headline + lead)
  pvalue_primary: "yes" / "no"  (a p-value in a sentence that mentions the
                  primary endpoint, or in the sentence right after it)
  pvalue_any:     "yes" / "no"  (any p-value in the body; exploratory only)
  phase:          "1", "1/2", "2", "2/3", "3", "4" or "unknown"

There is no LLM API key in this environment, so the "prompt" is this rule set.
It is frozen by hashing this file (freeze_classifier.py) before any return is
computed; the backtest refuses to run if the hash changes.
"""
import re

LEAD_CHARS = 3000

# ---- redaction ----------------------------------------------------------------
_MONTHS = (r"(?:Jan(?:uary)?|Feb(?:ruary)?|Mar(?:ch)?|Apr(?:il)?|May|June?|July?|"
           r"Aug(?:ust)?|Sep(?:t(?:ember)?)?|Oct(?:ober)?|Nov(?:ember)?|Dec(?:ember)?)\.?")
_MONTHS_ANYCASE = "(?i:" + _MONTHS[3:]
_REDACT = [
    # document file names often embed the ticker (e.g. abcd-ex99_1.htm)
    (re.compile(r"\S+\.(?:htm|html|txt|pdf|jpg|gif|png)\b", re.I), "[FILE]"),
    # (Nasdaq: ABCD), NYSE American: XYZ, OTCQB:ABCD, TSX: ABC ...
    (re.compile(r"\b(?:NASDAQ\w*|Nasdaq\w*|NYSE\w*(?:\s+(?:American|MKT|Arca))?|OTC\w*|TSX(?:-?V)?|"
                r"ASX|AIM|LSE|Euronext(?:\s+\w+)?|SIX|TASE)\s*(?:Global\s+\w+\s+Market|"
                r"Capital Market|Stock Market)?\s*:\s*[A-Z][A-Z.\-]{0,6}\b"), "[TICKER]"),
    (re.compile(_MONTHS_ANYCASE + r"\s+\d{1,2}(?:st|nd|rd|th)?,?\s+(?:19|20)\d{2}"), "[DATE]"),
    (re.compile(_MONTHS + r"\s+\d{1,2}(?:st|nd|rd|th)?\b"), "[DATE]"),
    (re.compile(r"\b\d{1,2}/\d{1,2}/(?:19|20)?\d{2}\b"), "[DATE]"),
    (re.compile(r"(?<![\d.])(?:19|20)\d{2}(?![\d]|\.\d)"), "[YEAR]"),
    (re.compile(r"(?:US)?\$\s?\d[\d,]*(?:\.\d+)?(?:\s*(?:million|billion|thousand|[MBK])\b)?"),
     "[AMOUNT]"),
]


def clean(text):
    text = (text.replace(" ", " ").replace("≤", "<=").replace("≥", ">=")
            .replace("‑", "-").replace("‐", "-").replace("–", "-")
            .replace("—", "-").replace("’", "'").replace("“", '"')
            .replace("”", '"').replace("＜", "<").replace("−", "-"))
    return re.sub(r"\s+", " ", text).strip()


def redact(text):
    text = clean(text)
    for pat, repl in _REDACT:
        text = pat.sub(repl, text)
    return text


# ---- text sections --------------------------------------------------------
_FLS = re.compile(r"(forward[- ]looking statements?|safe harbor|cautionary (?:note|statement))",
                  re.I)


def body(text):
    """Drop the forward-looking-statement boilerplate (and what follows it):
    it is full of hypotheticals such as 'may fail to meet the primary endpoint'."""
    for m in _FLS.finditer(text):
        if m.start() > 600:
            return text[:m.start()]
    return text


def sentences(text):
    parts = re.split(r"(?<=[.!?])\s+(?=[A-Z•\-\(\[\"])|\s*[•●▪◦■·]\s*", text)
    return [p for p in parts if p and p.strip()]


# ---- patterns -------------------------------------------------------------
PRIMARY = re.compile(r"\b(?:co-)?primary\s+(?:efficacy\s+|safety\s+and\s+efficacy\s+|"
                     r"composite\s+|clinical\s+)?(?:end\s?-?points?|objectives?|"
                     r"outcome(?:\s+measures?)?s?|analys[ie]s|efficacy\s+measures?)\b", re.I)
PVALUE = re.compile(r"(?<![a-z])p\s*-?\s*(?:values?\s*)?(?:[<=>]{1,2}|of|was|were|less than|"
                    r"below)\s*(?:0?\.\d|\d(?:\.\d+)?\s*[x×*]\s*10|\d\.?\d*\s*e\s*-)",
                    re.I)

# "primary" must be followed by an endpoint word: avoids disease names such as
# "primary dysmenorrhea" or "primary biliary cholangitis"
_ENDPOINT = (r"(?:and\s+(?:key\s+)?secondary\s+)?(?:efficacy\s+|composite\s+|clinical\s+)?"
             r"(?:end\s?-?points?|objectives?|outcomes?|goals?|analys[ie]s|efficacy)\b")
# words allowed between "positive" and "results/data" ("positive Phase 2b topline
# data"), so that "positive in-class competitor data" or "positive recommendation
# by independent data monitoring committee" do not count
_DESCRIPTOR = (r"(?:top-?\s?line|interim|final|preliminary|initial|updated|additional|new|"
               r"clinical|pivotal|registrational|long-?term|durable|efficacy|safety|biomarker|"
               r"proof-of-concept|phase|i{1,3}[ab]?|iv|[1-4][ab]?|[1-4][ab]?/[1-4][ab]?|first|full|"
               r"primary|study|trial|cohort|histologic|\d+-(?:week|month|year)|one-year|two-year|"
               r"open-label|randomized|placebo-controlled|human|in-human|first-in-human|mid-stage|"
               r"late-stage|key|overall|survival|extension)")
_NEGATED = re.compile(r"\b(?:not|n't|never|fail(?:ed|s)?\s+to|whether|if|unless|designed\s+to|"
                      r"assum\w*|subject\s+to|contingent\s+on|upon|"
                      r"expected\s+to|expects?\s+to|potential(?:ly)?\s+to|aim(?:s|ed)?\s+to|"
                      r"would|could|may|might|will|intended\s+to|powered\s+to)\b[^.;]{0,25}$",
                      re.I)
POS = [
    re.compile(r"\b(?:met|meets|meeting|achieved|achieves|achieving|reached|reaches|hit|hits|"
               r"exceeded|surpassed|satisfied)\b[^.;]{0,60}?\b(?:co-)?primary\s+" + _ENDPOINT, re.I),
    re.compile(r"\bprimary\s+(?:efficacy\s+)?(?:end\s?-?points?|objectives?|outcomes?)\b"
               r"[^.;]{0,40}?\b(?:was|were|has been|have been)\s+(?:successfully\s+)?"
               r"(?:met|achieved|reached)\b", re.I),
    re.compile(r"(?<![\w-])positive\s+(?:" + _DESCRIPTOR + r"\s+){0,3}?(?:top-?\s?line|results?|"
               r"data|read-?\s?outs?)\b", re.I),
    re.compile(r"\bsuccessful(?:ly)?\s+(?:[\w/-]+\s+){0,2}?(?:trial|study|results?|met)\b", re.I),
]
NEG = [
    re.compile(r"\b(?:did not|does not|didn't|failed to|fails to|not)\s+(?:\w+\s+)?"
               r"(?:meet|achieve|reach|demonstrate|show|attain|hit)\b[^.;]{0,80}?"
               r"\b(?:(?:co-)?primary\s+" + _ENDPOINT + r"|statistical(?:ly)?\s+significan)", re.I),
    re.compile(r"\bprimary\s+(?:efficacy\s+)?(?:end\s?-?points?|objectives?|outcomes?)\b"
               r"[^.;]{0,40}?\b(?:was|were)\s+not\s+(?:met|achieved|reached)\b", re.I),
    re.compile(r"\b(?:missed|failed)\b[^.;]{0,30}?\b(?:co-)?primary\s+" + _ENDPOINT, re.I),
    re.compile(r"\b(?:for|due to|because of|on)\s+(?:the\s+basis\s+of\s+)?futility\b|"
               r"\bfutility\s+(?:boundary|threshold|criteria|criterion)\s+(?:was|were|had been|"
               r"has been)\s+(?:met|crossed|reached)", re.I),
    re.compile(r"\b(?:negative|disappointing)\s+(?:[\w/-]+\s+){0,2}?(?:top-?\s?line|results?|"
               r"data|outcomes?)\b", re.I),
    re.compile(r"\bdid not (?:separate|differentiate)\b", re.I),
]
SIG_PRIMARY = re.compile(r"\bstatistically\s+significant", re.I)
NOT_SIG = re.compile(r"\b(?:not|no|non-?)\s*statistically\s+significant", re.I)
SECONDARY = re.compile(r"\b(?:secondary|exploratory|subgroup|post[- ]hoc)\b", re.I)

PHASE = re.compile(r"\bphase\s*(1|2|3|4|IV|III|II|I)[abAB]?(?![A-Za-z0-9])"
                   r"(?:\s*(?:/|-|and)\s*(?:phase\s*)?(1|2|3|III|II|I)[abAB]?(?![A-Za-z0-9]))?",
                   re.I)
_ROMAN = {"I": "1", "II": "2", "III": "3", "IV": "4"}


# A success claim must be news in this release, not a recap of older news.
_RECAP = re.compile(r"\b(?:previously|recently|earlier|last\s+(?:year|quarter|month)|"
                    r"based\s+on|supported\s+by|building\s+on|on\s+the\s+heels|"
                    r"in\s+(?:january|february|march|april|may|june|july|august|september|"
                    r"october|november|december|\[date\]|\[year\]|q[1-4]|the\s+(?:first|"
                    r"second|third|fourth)\s+quarter)|(?:announced|reported|presented|"
                    r"shared|delivered)|following)\b[^.;]{0,60}$", re.I)
_TODAY = re.compile(r"\btoday\b", re.I)
_HEDGED = re.compile(r"\b(?:if|whether|unless|risks?|could|may|might|in the event)\b[^.;]{0,40}$",
                     re.I)
HEADLINE_CHARS = 1000


def _positive_hit(pat, sent):
    today = bool(_TODAY.search(sent))
    for m in pat.finditer(sent):
        before = sent[:m.start()]
        if _NEGATED.search(before):
            continue
        if _RECAP.search(before) and not today:
            continue
        return True
    return False


def _sig_primary_hit(sent):
    m = SIG_PRIMARY.search(sent)
    return bool(m and PRIMARY.search(sent) and not NOT_SIG.search(sent)
                and not _NEGATED.search(sent[:m.start()])
                and (not _RECAP.search(sent[:m.start()]) or _TODAY.search(sent)))


def success_evidence(lead):
    """Sentences supporting / contradicting a success claim.
    Positive claims count in the headline area or in a 'today ...' sentence;
    failure statements count anywhere in the lead."""
    pos, neg = [], []
    offset = 0
    for s in sentences(lead):
        start = lead.find(s, offset)
        offset = max(offset, start)
        in_scope = start < HEADLINE_CHARS or _TODAY.search(s)
        if in_scope and (any(_positive_hit(p, s) for p in POS) or _sig_primary_hit(s)):
            pos.append(s)
        failed = NOT_SIG.search(s) and PRIMARY.search(s)
        for p in NEG:
            m = p.search(s)
            # a miss on a secondary/subgroup endpoint is not a failed trial
            if m and not _HEDGED.search(s[:m.start()]) and (
                    PRIMARY.search(s) or not SECONDARY.search(s)):
                failed = True
        if failed:
            neg.append(s)
    return pos, neg


def claims_success(lead):
    pos, neg = success_evidence(lead)
    if pos and not neg:
        return "yes"
    if neg and not pos:
        return "no"
    return "unclear"


def pvalue_evidence(text):
    sents = sentences(text)
    hits = []
    for i, s in enumerate(sents):
        if not PRIMARY.search(s):
            continue
        if PVALUE.search(s):
            hits.append(s)
        nxt = sents[i + 1] if i + 1 < len(sents) else ""
        if PVALUE.search(nxt) and not SECONDARY.search(nxt):
            hits.append(s + " || " + nxt)
    return hits


def pvalue_primary(text):
    return "yes" if pvalue_evidence(text) else "no"


def phase(lead, text):
    for chunk in (lead, text):
        m = PHASE.search(chunk)
        if m:
            a = _ROMAN.get(m.group(1).upper(), m.group(1))
            b = m.group(2)
            if b:
                b = _ROMAN.get(b.upper(), b)
                if b != a:
                    return f"{min(a, b)}/{max(a, b)}"
            return a
    return "unknown"


# Quarterly/annual results releases recap older trial news; like Item 2.02
# filings they are not trial-result announcements.
_EARNINGS = re.compile(r"\b(?:quarter|full[- ]year|fiscal(?:\s+year)?|year[- ]end)\b[^.]{0,80}?"
                       r"\b(?:financial\s+results|results|financial\s+highlights)\b", re.I)
EARNINGS_HEADLINE_CHARS = 400


def classify(redacted_text):
    """The only entry point. Input: redacted press-release text, nothing else."""
    text = body(redacted_text)
    lead = text[:LEAD_CHARS]
    earnings = bool(_EARNINGS.search(text[:EARNINGS_HEADLINE_CHARS]))
    return {"claims_success": "unclear" if earnings else claims_success(lead),
            "earnings_release": earnings,
            "pvalue_primary": pvalue_primary(text),
            # looser label for an exploratory variant: any p-value in the body
            "pvalue_any": "yes" if PVALUE.search(text) else "no",
            "phase": phase(lead, text)}
