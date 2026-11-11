#!/usr/bin/env python3
"""Cross-reference and drafting checks for the Code général des impôts.

Input : data/versions.jsonl (corpus/legi.py), data/index/*.tsv (corpus/extract_legi.py)
Output: findings as JSON lines, one per suspected fault.

Checks
------
R1  reference to an article of the CGI / annexes / LPF that never existed
R2  reference to an article that exists but is not in force at the check date
    (abrogé, transféré, périmé, disjoint) — with the repealing text ("blame")
R3  reference to an article of another code that does not exist / is not in force
S1  reference to a subdivision (I, A, 1, 1°, a…) that the cited article lacks
S2  reference to the n-th alinéa of a subdivision that has fewer alinéas
L1  "loi n° AAAA-NNN du …" whose number and date disagree (year prefix or index)
P1  unbalanced parentheses in an article
P2  unbalanced « guillemets »
D1  duplicated subdivision label at the same level of one article
D2  two articles in force with the same number in the same text
"""
from __future__ import annotations

import bisect
import collections
import csv
import json
import re
import sys
import unicodedata
from dataclasses import dataclass, field, asdict

LATIN = ("bis ter quater quinquies sexies septies octies nonies decies undecies duodecies "
         "terdecies quaterdecies quindecies sexdecies septdecies octodecies novodecies "
         "vicies unvicies duovicies tervicies quatervicies quinvicies sexvicies septvicies "
         "octovicies novovicies tricies untricies duotricies").split()
LATIN_RE = "|".join(sorted(LATIN, key=len, reverse=True))
MOIS = {m: i + 1 for i, m in enumerate(
    "janvier février mars avril mai juin juillet août septembre octobre novembre décembre".split())}

TEXT_NAMES = {
    "CGI": "code général des impôts",
    "CGI-AN1": "annexe I au code général des impôts",
    "CGI-AN2": "annexe II au code général des impôts",
    "CGI-AN3": "annexe III au code général des impôts",
    "CGI-AN4": "annexe IV au code général des impôts",
    "LPF": "livre des procédures fiscales",
}
TEXT_CID = {"CGI": "LEGITEXT000006069577", "CGI-AN1": "LEGITEXT000006069568",
            "CGI-AN2": "LEGITEXT000006069569", "CGI-AN3": "LEGITEXT000006069574",
            "CGI-AN4": "LEGITEXT000006069576", "LPF": "LEGITEXT000006069583"}
IN_FORCE_STATES = {"VIGUEUR", "VIGUEUR_DIFF", "ABROGE_DIFF", "MODIFIE"}


# --------------------------------------------------------------------------
# Article numbers
# --------------------------------------------------------------------------

def norm_num(s: str) -> str:
    """Canonical form of an article number: 'L. 16 B' -> 'L16 B', '1er' -> '1'."""
    s = unicodedata.normalize("NFKC", s)
    s = s.replace("‑", "-").replace("‐", "-").replace("–", "-").replace(" ", " ")
    s = re.sub(r"\s*-\s*", "-", s)
    s = re.sub(r"^(L\.?\s?O|[LRDA])\s?(\*?)\s?\.?\s?(\*?)\s*(?=\d)", lambda m: m.group(1).replace(".", "").replace(" ", "") + m.group(2) + m.group(3), s)
    s = re.sub(r"^(\d+)er\b", r"\1", s)
    s = re.sub(r"\s+", " ", s).strip()
    return s


@dataclass
class Version:
    id: str
    etat: str
    debut: str
    fin: str


class ArticleTable:
    """All versions of all articles of one text, indexed by normalized number."""

    def __init__(self):
        self.by_num: dict[str, list[Version]] = collections.defaultdict(list)

    def add(self, num: str, v: Version):
        self.by_num[norm_num(num)].append(v)

    def status(self, num: str, date: str) -> tuple[str, Version | None]:
        """('in_force'|'not_in_force'|'never', version in force or last version before date)."""
        vs = self.by_num.get(num)
        if not vs:
            return "never", None
        cur = [v for v in vs if v.debut <= date < v.fin and v.etat not in ("MODIFIE_MORT_NE", "ANNULE")]
        if cur:
            return "in_force", cur[0]
        past = sorted([v for v in vs if v.debut <= date], key=lambda v: v.fin)
        if past:
            return "not_in_force", past[-1]
        fut = sorted(vs, key=lambda v: v.debut)
        return "future", fut[0]

    def exists_prefix(self, toks: list[str]) -> int:
        """Length of the longest token prefix that names a known article."""
        for k in range(len(toks), 0, -1):
            if norm_num(" ".join(toks[:k])) in self.by_num:
                return k
        return 0


# --------------------------------------------------------------------------
# Article outline (subdivisions)
# --------------------------------------------------------------------------

DASH = r"(?:\s*[–\-—]\s*|\s+|$|(?=\d+°)|(?<=\s))"
LBL_ROMAN = rf"(?P<roman>[IVX]+(?:\s(?:{LATIN_RE}))?)\s?\.(?=\s*[–\-—]|\s|$)"
LBL_CAP = rf"(?P<cap>[A-Z](?:\s(?:{LATIN_RE}))?)\s?\.(?=\s*[–\-—]|\s)"
LBL_ARAB = rf"(?P<arab>\d+(?:\s(?:{LATIN_RE}))?)(?:\s?\.(?=\s*[–\-—]|\s|\d+°|[IVX]+\.)|\s(?=[A-ZÀÉÈÊ][a-zéèêàâîôû]))"
LBL_ORD = rf"(?P<ord>\d+°(?:\s(?:{LATIN_RE}))?)\.?"
LBL_LOW = rf"(?P<low>[a-z]{{1,2}}(?:\s(?:{LATIN_RE}))?)\s?[\.\)](?=\s)"
LABEL_RE = re.compile(rf"^\s*(?:{LBL_ROMAN}|{LBL_CAP}|{LBL_ARAB}|{LBL_ORD}|{LBL_LOW}){DASH}")


def roman_ok(s: str) -> bool:
    return re.fullmatch(r"M{0,3}(CM|CD|D?C{0,3})(XC|XL|L?X{0,3})(IX|IV|V?I{0,3})", s) is not None


@dataclass
class Label:
    kind: str
    label: str
    alinea: int


def outline(alineas: list[str]) -> list[Label]:
    """Labels found at the start of alinéas (several may be stacked: 'I. – A. – 1. …')."""
    out = []
    for i, a in enumerate(alineas):
        s = a
        for _ in range(4):
            m = LABEL_RE.match(s)
            if not m:
                break
            kind = next(k for k in ("roman", "cap", "arab", "ord", "low") if m.group(k))
            lab = re.sub(r"\s+", " ", m.group(kind)).strip()
            if kind == "roman" and not roman_ok(lab.split(" ")[0]):
                break
            if kind == "cap" and roman_ok(lab.split(" ")[0]) and lab.split(" ")[0] in ("I", "V", "X"):
                kind = "roman"
            out.append(Label(kind, lab, i))
            s = s[m.end():]
    return out


def sub_kind(lab: str) -> str:
    if re.fullmatch(rf"[IVX]+(?:\s(?:{LATIN_RE}))?", lab) and roman_ok(lab.split(" ")[0]):
        return "roman"
    if re.fullmatch(rf"[A-Z](?:\s(?:{LATIN_RE}))?", lab):
        return "cap"
    if re.fullmatch(rf"\d+°(?:\s(?:{LATIN_RE}))?", lab):
        return "ord"
    if re.fullmatch(rf"\d+(?:\s(?:{LATIN_RE}))?", lab):
        return "arab"
    if re.fullmatch(rf"[a-z]{{1,2}}(?:\s(?:{LATIN_RE}))?", lab):
        return "low"
    return "?"


def find_range(labels: list[Label], n_alineas: int, chain: list[str]) -> tuple[bool, int, int, str]:
    """Resolve an outermost-first chain of labels ['II', '2°', 'a'] in an outline.

    Returns (found, start_alinea, end_alinea_exclusive, missing_label)."""
    lo, hi = 0, n_alineas
    for lab in chain:
        k = sub_kind(lab)
        if k == "?":
            return True, lo, hi, ""
        cands = [x for x in labels if x.kind == k and lo <= x.alinea < hi]
        hit = [x for x in cands if x.label == lab]
        if not hit:
            # roman labels are sometimes typed as capitals ('V.'), or arabic as ordinal
            alt = [x for x in labels if lo <= x.alinea < hi and x.label == lab]
            if not alt:
                return False, lo, hi, lab
            hit = alt
            cands = [x for x in labels if x.kind == alt[0].kind and lo <= x.alinea < hi]
        start = hit[0].alinea
        nxt = [x.alinea for x in cands if x.alinea > start and x.label != lab]
        end = min(nxt) if nxt else hi
        lo, hi = start, end
    return True, lo, hi, ""


# --------------------------------------------------------------------------
# Citation extraction
# --------------------------------------------------------------------------

SUB_TOKEN = (rf"(?:[IVX]+(?:\s(?:{LATIN_RE}))?|[A-Z](?:\s(?:{LATIN_RE}))?|\d+°?(?:\s(?:{LATIN_RE}))?"
             rf"|[a-z]{{1,2}}(?:\s(?:{LATIN_RE}))?)")
ORDINALS = {"premier": 1, "première": 1, "deuxième": 2, "second": 2, "seconde": 2, "troisième": 3,
            "quatrième": 4, "cinquième": 5, "sixième": 6, "septième": 7, "huitième": 8,
            "neuvième": 9, "dixième": 10, "onzième": 11, "douzième": 12, "treizième": 13,
            "quatorzième": 14, "quinzième": 15, "seizième": 16, "dix-septième": 17,
            "dix-huitième": 18, "dix-neuvième": 19, "vingtième": 20}
ALINEA_TOKEN = r"(?:(?P<ord_word>" + "|".join(sorted(ORDINALS, key=len, reverse=True)) + r"|dernier|avant-dernier)\s+alinéa)"
PREP = r"(?<![\w'])(?:aux?|du|des|à\s+la|à\s+l'|de\s+la|de\s+l'|le|la|les|l')"

# a chain like "au a du 2° du I de l'article" or "au premier alinéa du 1 de l'article"
CHAIN_RE = re.compile(
    rf"(?P<chain>(?:{PREP}\s*(?:{ALINEA_TOKEN}|{SUB_TOKEN}(?:\s*(?:,|et|ou)\s*{SUB_TOKEN})*)\s+)+)"
    rf"(?P<tail>de\s+l'article|des\s+articles|du\s+présent\s+article|du\s+même\s+article|dudit\s+article|de\s+cet\s+article)\b")

ART_KW = re.compile(r"\b(?P<kw>articles?|art\.)\s+(?=(?:L\.?\s?O\.?|[LRDA])?\s?\*?\.?\s?\d)")
NUM_TOKEN = re.compile(rf"\s*(?P<t>(?:L\.?\s?O\.?|[LRDA])\s?\*?\.?\s?\d+(?:-\d+)*|\d+(?:er)?(?:-\d+)*|(?:{LATIN_RE})\b|[A-Z]{{1,3}}\b(?!')|-0\b|-\d+\b)")
LIST_SEP = re.compile(r"\s*(?:,|\bet\b|\bou\b|\bà\b)\s*(?=(?:L\.?\s?O\.?|[LRDA])?\s?\*?\.?\s?[1-9])")

SKIP_BEFORE_QUAL = re.compile(r"^\s*(?:,?\s*(?:et\s+suivants|précitée?s?|susvisée?s?|susmentionnée?s?|ci-dessus|ci-après|modifiée?s?)\b|\s*\([^()]{0,80}\))+")

QUAL_PATTERNS = [
    (re.compile(r"^\s*,?\s*(?:du|au|dudit|le)\s+présent\s+code\b"), "SELF_CODE"),
    (re.compile(r"^\s*,?\s*de\s+la\s+présente\s+annexe\b"), "SELF"),
    (re.compile(r"^\s*,?\s*(?:de\s+l'|à\s+l')annexe\s+(?P<an>I{1,3}|IV)\s+(?:au|du)\s+(?:présent\s+code|même\s+code|code\s+général\s+des\s+impôts)"), "ANNEX"),
    (re.compile(r"^\s*,?\s*(?:du|au)\s+code\s+général\s+des\s+impôts\b"), "CGI"),
    (re.compile(r"^\s*,?\s*du\s+livre\s+des\s+procédures\s+fiscales\b"), "LPF"),
    (re.compile(r"^\s*,?\s*(?:(?:du|au)\s+(?:même|dit)\s+(?:code|livre)|de\s+ce\s+(?:code|livre)|(?:dudit|audit)\s+(?:code|livre)|(?:du|au)\s+(?:code|livre)\s+précité)\b"), "SAME_CODE"),
    (re.compile(r"^\s*,?\s*de\s+la\s+même\s+annexe\b"), "SAME_CODE"),
    (re.compile(r"^\s*,?\s*(?:du|au)\s+(?P<code>code\s.*)"), "CODE"),
    (re.compile(r"^\s*,?\s*(?:de\s+la|à\s+la)\s+(?:loi|ordonnance|constitution|directive|décision|convention)\b"), "EXTERNAL"),
    (re.compile(r"^\s*,?\s*(?:du|au)\s+(?:décret|règlement|traité|protocole|arrêté)\b"), "EXTERNAL"),
    (re.compile(r"^\s*,?\s*(?:de\s+l'|à\s+l')(?:ordonnance|accord|arrêté|annexe\s+(?!I{1,3}\b|IV\b))"), "EXTERNAL"),
    (re.compile(r"^\s*,?\s*des\s+(?:statuts|conventions)\b"), "EXTERNAL"),
    (re.compile(r"^\s*,?\s*(?:de\s+la\s+même\s+loi|du\s+même\s+décret|de\s+ladite\s+loi|de\s+la\s+même\s+ordonnance|du\s+même\s+règlement|de\s+la\s+même\s+directive|de\s+(?:ladite|cette)\s+(?:loi|ordonnance|directive|convention|décision)|dudit\s+(?:décret|règlement|accord|traité)|du\s+même\s+(?:accord|traité|arrêté))\b"), "EXTERNAL"),
]


@dataclass
class Ref:
    alinea: int
    start: int
    raw: str
    nums: list[str]
    is_range: bool
    target: str          # CGI, CGI-AN2, LPF, CODE:<cid>, EXTERNAL, UNKNOWN_CODE:<name>
    explicit: bool = False
    chain: list[str] = field(default_factory=list)   # outermost first
    alinea_ref: int | None = None                     # n-th alinéa (−1 = dernier)


def read_num_list(s: str, pos: int) -> tuple[list[list[str]], bool, int]:
    """Read 'NUM (, NUM)* (et|à NUM)?' from s[pos:]. Returns (token lists, has_range, end)."""
    nums, is_range = [], False
    while True:
        toks, p = [], pos
        while True:
            m = NUM_TOKEN.match(s, p)
            if not m:
                break
            t = m.group("t")
            if toks and re.match(r"(?:L\.?\s?O\.?|[LRDA])\s?\*?\.?\s?\d|\d", t) and not t.startswith("-"):
                break  # a new number starts: belongs to the list separator logic
            if not toks and not re.match(r"(?:L\.?\s?O\.?|[LRDA])\s?\*?\.?\s?\d|\d", t):
                break
            toks.append(t)
            p = m.end()
        if not toks:
            break
        nums.append(toks)
        pos = p
        m = LIST_SEP.match(s, pos)
        if not m:
            break
        if re.search(r"\bà\b", m.group(0)):
            is_range = True
        pos = m.end()
    return nums, is_range, pos


def ordinal_of(word: str) -> int:
    if word == "dernier":
        return -1
    if word == "avant-dernier":
        return -2
    return ORDINALS[word]


def parse_chain(chain_text: str) -> tuple[list[str], int | None]:
    """'au a du 2° du I ' -> (['I', '2°', 'a'], None); alinéa words become alinea_ref."""
    parts = re.split(rf"\b{PREP}\s*", chain_text)
    labels, alinea = [], None
    for p in parts:
        p = p.strip()
        if not p:
            continue
        m = re.fullmatch(ALINEA_TOKEN, p)
        if m:
            alinea = ordinal_of(m.group("ord_word") or ("dernier" if p.startswith("dernier") else "avant-dernier"))
            continue
        first = re.split(r"\s*(?:,|\bet\b|\bou\b)\s*", p)[0].strip()
        if re.fullmatch(SUB_TOKEN, first):
            labels.append(first)
    return list(reversed(labels)), alinea


class CodeNames:
    """Map 'code de la sécurité sociale' → LEGITEXT id, from the code article index."""

    def __init__(self, titles: dict[str, str]):
        self.titles = {}
        for cid, t in titles.items():
            k = self.key(t)
            self.titles[k] = cid
        self.keys = sorted(self.titles, key=len, reverse=True)

    @staticmethod
    def key(t: str) -> str:
        t = unicodedata.normalize("NFKC", t).lower().replace("’", "'")
        t = re.sub(r"\s*\(.*?\)\s*", " ", t)
        return re.sub(r"\s+", " ", t).strip()

    def resolve(self, phrase: str) -> str | None:
        p = self.key(phrase)
        for k in self.keys:
            if p.startswith(k) and (len(p) == len(k) or not p[len(k)].isalpha()):
                return self.titles[k]
        return None


CODE_MENTION = re.compile(r"\b(?:code\s+général\s+des\s+impôts|livre\s+des\s+procédures\s+fiscales|code\s+[^\s,;.()]+)", re.I)


def mention_target(a: str, pos: int, codes: CodeNames, text_label: str) -> str | None:
    """Text named most recently before pos (for 'dudit code', 'même livre', 'précité')."""
    best = None
    for m in CODE_MENTION.finditer(a, 0, pos):
        w = m.group(0).lower()
        if w.startswith("code général des impôts"):
            best = "CGI"
        elif w.startswith("livre des"):
            best = "LPF"
        else:
            cid = codes.resolve(a[m.start():])
            if cid:
                best = "CGI" if cid == TEXT_CID["CGI"] else "LPF" if cid == TEXT_CID["LPF"] else "CODE:" + cid
    return best


FWD_QUAL = re.compile(r"\b(?:du|de\s+ce|dudit|du\s+même)\s+(?:code\s+général\s+des\s+impôts|livre\s+des\s+procédures\s+fiscales|code\s+[^\s,;.()]+(?:\s+[^\s,;.()]+){0,8})")
SAME_ART = re.compile(r"(?:même|dudit|audit|ledit)\s*$")


def extract_refs(text_label: str, alineas: list[str], codes: CodeNames) -> list[Ref]:
    refs: list[Ref] = []
    last_code: str | None = None
    seen_num_target: dict[str, str] = {}
    for ai, a in enumerate(alineas):
        a = a.replace("’", "'")
        before_all = "\n".join(alineas[:ai]).replace("’", "'") + "\n"
        # 1. explicit article citations
        for m in ART_KW.finditer(a):
            start = m.end()
            nums, is_range, end = read_num_list(a, start)
            if not nums:
                continue
            after = a[end:end + 400]
            sk = SKIP_BEFORE_QUAL.match(after)
            if sk:
                after = after[sk.end():]
            target = text_label
            explicit = False
            for rx, kind in QUAL_PATTERNS:
                q = rx.match(after)
                if not q:
                    continue
                explicit = kind in ("CGI", "LPF", "ANNEX", "SELF_CODE", "SELF", "CODE")
                if kind == "SELF_CODE":
                    target = "CGI" if text_label.startswith("CGI") else text_label
                elif kind == "SELF":
                    target = text_label
                elif kind == "ANNEX":
                    target = {"I": "CGI-AN1", "II": "CGI-AN2", "III": "CGI-AN3", "IV": "CGI-AN4"}[q.group("an")]
                elif kind == "CGI":
                    target = "CGI"
                elif kind == "LPF":
                    target = "LPF"
                elif kind == "SAME_CODE":
                    target = (mention_target(before_all + a, len(before_all) + m.start(), codes, text_label)
                              or last_code or text_label)
                elif kind == "CODE":
                    cid = codes.resolve(q.group("code"))
                    target = ("CODE:" + cid) if cid else ("UNKNOWN_CODE:" + q.group("code"))
                    if cid == TEXT_CID["CGI"]:
                        target = "CGI"
                    elif cid == TEXT_CID["LPF"]:
                        target = "LPF"
                else:
                    target = "EXTERNAL"
                break
            else:
                # no explicit qualifier
                first = norm_num(" ".join(nums[0]))
                pre_kw = a[max(0, m.start() - 40):m.start()]
                sent_end = re.search(r"[.;](?:\s|$)", a[end:])
                sent_full = a[end:end + (sent_end.start() if sent_end else 600)]
                nxt_art = re.search(r"\barticles?\b", sent_full)
                upto_next = sent_full[:nxt_art.start()] if nxt_art else sent_full
                ext_fwd = re.search(r"\b(?:de\s+la|à\s+la)\s+(?:loi|directive|convention)\b|\b(?:du|au)\s+(?:décret|règlement)\b|\bde\s+l'(?:ordonnance|accord|arrêté)\b", upto_next)
                fq_any = FWD_QUAL.search(sent_full)
                if SAME_ART.search(pre_kw):
                    target = seen_num_target.get(first, "UNKNOWN_CODE:même article")
                elif re.search(r"(?:loi|ordonnance|décret)[^.;]{0,80},\s*$", pre_kw) and m.group("kw") == "art.":
                    target = "EXTERNAL"
                elif ext_fwd and not re.search(r"\bcode\b", upto_next[:ext_fwd.start()]):
                    target = "EXTERNAL"
                elif text_label == "LPF" and re.match(r"(?:L|R|D|LO)\*?\d", first) and fq_any and "procédures fiscales" not in fq_any.group(0):
                    w = fq_any.group(0)
                    if re.search(r"(?:de\s+ce|dudit|du\s+même)\s+code", w):
                        target = mention_target(before_all + a, len(before_all) + m.start(), codes, text_label) or text_label
                    else:
                        cid = codes.resolve(re.sub(r"^(?:du|de\s+ce|dudit|du\s+même)\s+", "", w))
                        target = ("CGI" if cid == TEXT_CID["CGI"] else "CODE:" + cid) if cid else text_label
                elif re.match(r"(?:L|R|D|LO)\*?\d", first) and text_label != "LPF":
                    sent_end = re.search(r"[.;](?:\s|$)", a[end:])
                    sent = a[end:end + (sent_end.start() if sent_end else 600)]
                    fq = FWD_QUAL.search(sent)
                    t2 = None
                    if fq:
                        w = fq.group(0)
                        if "général des impôts" in w:
                            t2 = "CGI"
                        elif "procédures fiscales" in w:
                            t2 = "LPF"
                        elif re.search(r"(?:de\s+ce|dudit|du\s+même)\s+code", w):
                            t2 = mention_target(before_all + a, len(before_all) + m.start(), codes, text_label)
                        else:
                            cid = codes.resolve(w.split(" ", 1)[1] if not w.startswith("de ce") else w)
                            if cid:
                                t2 = "CGI" if cid == TEXT_CID["CGI"] else "LPF" if cid == TEXT_CID["LPF"] else "CODE:" + cid
                    if t2 is None:
                        t2 = mention_target(before_all + a, len(before_all) + m.start(), codes, text_label)
                    target = t2 or "UNKNOWN_CODE:?"
            for toks in nums:
                seen_num_target[norm_num(" ".join(toks))] = target
            if target not in ("EXTERNAL",):
                last_code = target
            # subdivision chain immediately before "article"
            pre = a[max(0, m.start() - 220):m.start() + len(m.group("kw"))]
            chain, alinea_ref = [], None
            cm = None
            for cm in CHAIN_RE.finditer(pre):
                pass
            if cm and cm.end() >= len(pre) - 1:
                chain, alinea_ref = parse_chain(cm.group("chain"))
            refs.append(Ref(ai, m.start(), a[m.start():end].strip(), [" ".join(t) for t in nums],
                            is_range, target, explicit, chain if len(nums) == 1 else [], alinea_ref if len(nums) == 1 else None))
        # 2. self references: "au 2° du II du présent article"
        for cm in CHAIN_RE.finditer(a):
            if cm.group("tail").startswith("du présent"):
                chain, alinea_ref = parse_chain(cm.group("chain"))
                refs.append(Ref(ai, cm.start(), cm.group(0), ["<self>"], False, text_label, True, chain, alinea_ref))
    return refs


LAW_RE = re.compile(
    r"\b(?P<nat>loi(?:\s+organique)?|ordonnance|décret)\s+n°\s?(?P<num>\d{2,4}-\d+)\s+du\s+"
    r"(?P<j>\d{1,2})(?:er)?\s+(?P<m>" + "|".join(MOIS) + r")\s+(?P<y>\d{4})")


# --------------------------------------------------------------------------
# Drafting checks
# --------------------------------------------------------------------------

def paren_balance(alineas: list[str]) -> tuple[int, int, int]:
    text = "\n".join(alineas)
    # ignore enumerations like "1)" "a)" at the start of alinéas, and smileys in tables
    t = re.sub(r"(?m)^\s*(?:[a-z]{1,2}|\d+|[ivx]+)\)", "", text)
    t = re.sub(r"(?<=\s)(?:[a-z]|\d{1,2})\)(?=\s)", "", t)
    return t.count("(") - t.count(")"), text.count("«"), text.count("»")


def duplicate_labels(labels: list[Label]) -> list[tuple[str, str]]:
    """Same label repeated under the same parent.  The nesting order of label
    kinds varies between articles (1° under a, or a under 1°), so it is
    inferred with a stack: a kind already on the stack closes every deeper level."""
    dups = []
    stack: list[tuple[str, set[str]]] = []
    for x in labels:
        idx = next((i for i, (k, _) in enumerate(stack) if k == x.kind), None)
        if idx is None:
            stack.append((x.kind, {x.label}))
            continue
        del stack[idx + 1:]
        if x.label in stack[idx][1]:
            dups.append((x.kind, x.label))
        stack[idx][1].add(x.label)
    return dups


# --------------------------------------------------------------------------
# Driver
# --------------------------------------------------------------------------

def load(versions_path: str, index_dir: str):
    tables = {k: ArticleTable() for k in TEXT_NAMES}
    versions = []
    for line in open(versions_path, encoding="utf-8"):
        r = json.loads(line)
        versions.append(r)
        tables[r["text"]].add(r["num"], Version(r["id"], r["etat"], r["debut"], r["fin"]))
    code_tables: dict[str, ArticleTable] = collections.defaultdict(ArticleTable)
    titles = {}
    with open(f"{index_dir}/code_articles.tsv", encoding="utf-8") as f:
        rd = csv.DictReader(f, delimiter="\t", quoting=csv.QUOTE_NONE)
        for row in rd:
            if row["code_titre"]:
                titles.setdefault(row["text_cid"], row["code_titre"])
            code_tables[row["text_cid"]].add(row["num"], Version(row["id"], row["etat"], row["date_debut"], row["date_fin"]))
    laws = collections.defaultdict(list)
    with open(f"{index_dir}/texts.tsv", encoding="utf-8") as f:
        rd = csv.DictReader(f, delimiter="\t", quoting=csv.QUOTE_NONE)
        for row in rd:
            if row["num"]:
                laws[row["num"]].append((row["nature"], row["date_texte"], row["cid"]))
    return versions, tables, code_tables, CodeNames(titles), titles, laws


def resolve_internal(tables, presumed: str, toks: str, date: str, explicit: bool = False) -> tuple[str, str, str]:
    """Resolve a cited number to (text, normalized number, how)."""
    tl = toks.split(" ")
    full = norm_num(toks)
    order = [presumed]
    if explicit:
        for cand in (full, re.sub(r"^R(?=\d)", "R*", full)):
            if cand in tables[presumed].by_num:
                return presumed, cand, "exact"
        k = tables[presumed].exists_prefix(tl)
        return (presumed, norm_num(" ".join(tl[:k])), "prefix") if k else (presumed, full, "unresolved")
    prefixed = re.match(r"(?:L|R|D|A|LO)\*?\d", full) is not None
    if presumed != "CGI" and not prefixed:
        order.append("CGI")         # annexes and LPF citing the code without qualifier
    if presumed.startswith("CGI") and prefixed:
        order.append("LPF")
    if presumed == "CGI":
        order += ["CGI-AN2", "CGI-AN3", "CGI-AN4"]
    elif presumed.startswith("CGI-AN"):
        order += [t for t in ("CGI-AN1", "CGI-AN2", "CGI-AN3", "CGI-AN4") if t != presumed]
    # 1. exact, in force somewhere
    for t in order:
        for cand in (full, re.sub(r"^R(?=\d)", "R*", full)):
            if tables[t].status(cand, date)[0] == "in_force":
                return t, cand, "exact" if t == presumed else "implicit:" + t
    # 2. exact, known (not in force) in the presumed text, then elsewhere
    for t in order:
        for cand in (full, re.sub(r"^R(?=\d)", "R*", full)):
            if cand in tables[t].by_num:
                return t, cand, "exact" if t == presumed else "implicit:" + t
    # 3. longest known prefix
    for t in order:
        k = tables[t].exists_prefix(tl)
        if k:
            return t, norm_num(" ".join(tl[:k])), "prefix"
    return presumed, full, "unresolved"


def blame(v: dict) -> str:
    mods = [l for l in v["liens"] if l["sens"] == "cible" and l["typelien"] in ("MODIFIE", "CREE", "CREATION", "MODIFICATION", "ABROGE", "TRANSFERE")]
    return "; ".join(sorted({l["label"] for l in mods}))[:300]


def run(date: str, versions, tables, code_tables, codes, titles, laws, by_id) -> list[dict]:
    findings = []
    in_force = [v for v in versions if v["debut"] <= date < v["fin"] and v["etat"] not in ("MODIFIE_MORT_NE", "ANNULE")]
    nums_in_force = collections.Counter((v["text"], norm_num(v["num"])) for v in in_force)
    current = {(v["text"], norm_num(v["num"])): v for v in in_force}

    def add(kind, v, **kw):
        findings.append({"check": kind, "text": v["text"], "article": v["num"], "version": v["id"],
                         "version_debut": v["debut"], **kw})

    for (t, n), c in nums_in_force.items():
        if c > 1:
            ids = [v["id"] for v in in_force if v["text"] == t and norm_num(v["num"]) == n]
            add("D2", current[(t, n)], detail=f"{c} versions en vigueur portent le numéro {n}", ids=ids)

    for v in in_force:
        al = v["alineas"]
        labels = outline(al)
        # P1/P2
        bal, lq, rq = paren_balance(al)
        if bal != 0:
            add("P1", v, detail=f"parenthèses déséquilibrées ({bal:+d})")
        if lq != rq:
            add("P2", v, detail=f"guillemets déséquilibrés (« {lq} / » {rq})")
        for kind, lab in duplicate_labels(labels):
            add("D1", v, detail=f"subdivision « {lab} » ({kind}) répétée au même niveau")
        # L1
        for ai, a in enumerate(al):
            for m in LAW_RE.finditer(a):
                num, y = m.group("num"), m.group("y")
                d = f"{y}-{MOIS[m.group('m')]:02d}-{int(m.group('j')):02d}"
                pre = num.split("-")[0]
                year_ok = (len(pre) == 4 and pre == y) or (len(pre) == 2 and y.endswith(pre))
                hits = [h for h in laws.get(num, []) if h[0].lower().startswith(m.group("nat").split()[0][:3].lower())]
                exact = [h for h in hits if h[1] == d]
                if not year_ok and not exact:
                    add("L1", v, alinea=ai, raw=m.group(0), detail=f"numéro {num} incompatible avec l'année {y}",
                        index=[h[1] for h in hits])
                elif hits and not exact:
                    add("L1", v, alinea=ai, raw=m.group(0),
                        detail=f"{m.group('nat')} n° {num} datée du {d} dans le texte, du {', '.join(sorted({h[1] for h in hits}))} au JO",
                        index=[h[1] for h in hits])
        # references
        for r in extract_refs(v["text"], al, codes):
            if r.nums == ["<self>"]:
                if r.chain:
                    ok, lo, hi, miss = find_range(labels, len(al), r.chain)
                    if not ok:
                        add("S1", v, alinea=r.alinea, raw=r.raw, target_article=v["num"], target_text=v["text"],
                            chain=r.chain, missing=miss, detail=f"« {miss} » introuvable dans le présent article")
                continue
            if r.target.startswith("UNKNOWN_CODE") or r.target == "EXTERNAL":
                continue
            if r.target.startswith("CODE:"):
                cid = r.target[5:]
                tab = code_tables.get(cid)
                if tab is None:
                    continue
                for toks in r.nums:
                    tl = toks.split(" ")
                    k = tab.exists_prefix(tl)
                    n = norm_num(" ".join(tl[:k])) if k else norm_num(toks)
                    st, ver = tab.status(n, date)
                    if st == "never":
                        add("R3", v, alinea=r.alinea, raw=r.raw, target_text=titles.get(cid, cid), target_article=n,
                            detail="article inexistant dans ce code")
                    elif st != "in_force":
                        add("R3", v, alinea=r.alinea, raw=r.raw, target_text=titles.get(cid, cid), target_article=n,
                            detail=f"article {ver.etat.lower()} depuis le {ver.fin}", target_version=ver.id)
                continue
            if r.target not in tables:
                continue
            for i, toks in enumerate(r.nums):
                tgt, n, how = resolve_internal(tables, r.target, toks, date, r.explicit)
                tab = tables[tgt]
                st, ver = tab.status(n, date)
                r_target = tgt
                if st == "never":
                    add("R1", v, alinea=r.alinea, raw=r.raw, target_text=r_target, target_article=n,
                        detail="article jamais créé dans ce texte")
                    continue
                if st != "in_force":
                    tv = by_id.get(ver.id)
                    add("R2", v, alinea=r.alinea, raw=r.raw, target_text=r_target, target_article=n,
                        detail=f"article {ver.etat.lower()} (dernière version jusqu'au {ver.fin})",
                        target_version=ver.id, target_blame=blame(tv) if tv else "",
                        context=al[r.alinea][:400])
                    continue
                if r.chain or r.alinea_ref:
                    tv = current.get((r_target, n))
                    if tv is None:
                        continue
                    tl_labels = outline(tv["alineas"])
                    ok, lo, hi, miss = find_range(tl_labels, len(tv["alineas"]), r.chain) if r.chain else (True, 0, len(tv["alineas"]), "")
                    if not ok:
                        add("S1", v, alinea=r.alinea, raw=r.raw, target_text=r_target, target_article=n,
                            chain=r.chain, missing=miss, detail=f"« {miss} » introuvable dans l'article {n}",
                            context=al[r.alinea][:400])
                    elif r.alinea_ref and r.alinea_ref > 0 and hi - lo < r.alinea_ref:
                        add("S2", v, alinea=r.alinea, raw=r.raw, target_text=r_target, target_article=n,
                            chain=r.chain, detail=f"alinéa n° {r.alinea_ref} demandé, {hi - lo} alinéa(s) présent(s)",
                            context=al[r.alinea][:400])
    return findings


def main():
    import argparse
    ap = argparse.ArgumentParser()
    ap.add_argument("--versions", default="data/versions.jsonl")
    ap.add_argument("--index", default="data/index")
    ap.add_argument("--date", default="2026-11-08")
    ap.add_argument("--out", default="-")
    a = ap.parse_args()
    versions, tables, code_tables, codes, titles, laws = load(a.versions, a.index)
    by_id = {v["id"]: v for v in versions}
    fs = run(a.date, versions, tables, code_tables, codes, titles, laws, by_id)
    out = sys.stdout if a.out == "-" else open(a.out, "w", encoding="utf-8")
    for f in fs:
        out.write(json.dumps(f, ensure_ascii=False) + "\n")
    print(collections.Counter(f["check"] for f in fs), file=sys.stderr)


if __name__ == "__main__":
    main()
