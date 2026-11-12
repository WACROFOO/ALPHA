# Formaliser le Code général des impôts — à la recherche des failles

Équivalent français du travail de Lexifina sur le Title 26 américain
([*Formalizing the US Tax Code*](https://lexifina.com/blog/formalizing-the-us-tax-code)) :
le **Code général des impôts** (CGI), ses annexes et le Livre des procédures fiscales
sont extraits du flux open data LEGI, l'impôt sur le revenu 2025 est modélisé en
**Lean 4** avec preuves, validé contre la brochure officielle et OpenFisca-France,
puis exploré pour trouver des **failles trop rentables qui devraient être corrigées**.

Outils en **Rust** (`tools/`), modèle et preuves en **Lean** (`lean/`). Seul
l'adaptateur OpenFisca (`validation/openfisca_run.py`) reste en Python, car
OpenFisca est une bibliothèque Python.

---

## 1. Failles à corriger (gains disproportionnés, sans substance économique)

Tous les montants sont calculés par le modèle Lean (`cgi-failles`, détail :
[`docs/failles_detail.md`](docs/failles_detail.md)), droit applicable aux revenus de 2025.

### F1 — La « règle du quart » neutralise la contribution différentielle sur les plus grosses plus-values

La CDHR (art. 224, créée par la LFI 2025 et prorogée par la LFI 2026) garantit un
taux minimal de 20 % aux revenus supérieurs à 250 000 € (500 000 € pour un couple).
Mais le 9ᵉ alinéa du II prévoit que les revenus « qui, par leur nature, ne sont pas
susceptibles d'être recueillis annuellement » et qui dépassent la moyenne des trois
années précédentes « sont retenus pour le quart de leur montant » — et le IV (A, 1° et
B, 3°) retient l'impôt et la CEHR correspondants pour le quart également. La
doctrine y range la plus-value de cession ponctuelle d'actions (BOI-IR-CDHR-10/20)
et, pour les revenus exceptionnels en général, les distributions de réserves.

| plus-value de cession (célibataire, 80 k€ de salaires) | impôt si plus-value ordinaire | impôt si « exceptionnelle » | gain | taux effectif |
|---|---:|---:|---:|---:|
| 1 M€ | 214 400 € | 178 185 € | **36 215 €** | 17,8 % au lieu de 20 % |
| 5 M€ | 1 014 400 € | 887 185 € | **127 215 €** | 17,7 % |
| 10 M€ | 2 014 400 € | 1 767 185 € | **247 215 €** | 17,7 % |
| 50 M€ | 10 014 400 € | 8 807 185 € | **1 207 215 €** | 17,6 % |

Le gain croît linéairement (≈ 2,4 % du montant) : le dispositif anti-optimisation
exempte précisément les revenus — cessions d'entreprise, distributions accumulées
dans une holding — qui forment l'essentiel des très hauts revenus. Aucun acte n'est
nécessaire hors l'étalement de la stratégie dans le temps.
**Correction proposée :** supprimer la règle du quart pour l'assiette de la CDHR
(ou la remplacer par un étalement sur quatre ans, comme le « lissage » de la CEHR),
et la réserver aux revenus effectivement imposés au barème selon le système du
quotient (art. 163-0 A), pas aux gains imposés au taux forfaitaire de 12,8 %.

### F2 — Un PACS avec un partenaire sans revenu double les seuils de la CEHR et de la CDHR

Les seuils de la CEHR (art. 223 sexies) et de la CDHR (art. 224) sont doublés pour un
couple soumis à imposition commune, quelle que soit la répartition des revenus, et la
CDHR accorde en plus 12 500 €. Un PACS se conclut et se dissout par simple déclaration.

| dividendes d'un contribuable seul (+ 60 k€ de salaires) | impôt seul + partenaire | impôt pacsés | gain annuel | dont CEHR + CDHR |
|---|---:|---:|---:|---:|
| 300 k€ | 70 800 € | 41 788 € | **29 012 €** | 23 096 € |
| 400 k€ | 90 800 € | 54 588 € | **36 212 €** | 30 296 € |
| 500 k€ | 110 800 € | 69 008 € | **41 792 €** | 35 876 € |
| ≥ 800 k€ | — | — | **12 500 €** | l'abattement « couple » de la CDHR |

Plus de 80 % du gain ne vient pas du quotient conjugal de l'impôt sur le revenu mais
des contributions censées viser les très hauts revenus.
**Correction proposée :** pour la CEHR et la CDHR, fixer le seuil du couple à
250 000 € + le revenu du conjoint le moins rémunéré (plafonné à 250 000 €), et ne
réserver l'abattement de 12 500 € qu'aux couples dont chaque membre a des revenus.

### Moins rentables (signalées, pas prioritaires)

* **F3** — année du PACS : imposition séparée + donation de titres au partenaire : au plus
  ≈ 8 400 € et souvent perdant (perte de l'abattement de 12 500 €).
* **F4** — année du mariage : l'abattement de 10 % sur les pensions étant plafonné *par
  foyer* (4 439 €), l'imposition séparée le double : 1 332 à 1 819 €, une fois.
  (À l'inverse, ce plafond pénalise chaque année les couples de retraités mariés.)
* **F5** — pension de 6 855 € à un enfant majeur au lieu du rattachement : 250 à 1 483 €
  par an (connu, voulu par le législateur).
* **F6** — option pour le barème pour réduire l'assiette de la CDHR : jamais rentable.

### Failles profitables aux revenus modestes (prouvées en Lean)

`lean/CGI/IR2025/Modestes.lean` énonce chaque cas comme un théorème vérifié par
calcul sur le modèle ; `cgi-modestes` (Rust) en recherche les pires cas.

* **M1 — versement libératoire « décalé » (art. 151-0)** : l'accès dépend du seul
  RFR de l'année N-2 (29 579 € pour une personne seule). Un contribuable modeste
  deux ans plus tôt peut facturer jusqu'au plafond micro et ne payer que 1 % à
  2,2 % du chiffre d'affaires : **8 743 €/an** économisés au plafond des ventes
  (203 100 € : 10 774 € → 2 031 €), **7 818 €/an** au plafond BNC, pendant deux ans,
  alors que son RFR de l'année dépasse de 99 % la limite.
  *Correction :* apprécier aussi la condition sur le RFR de l'année (régularisation
  si elle est dépassée), ou plafonner le chiffre d'affaires éligible.
* **M2 — décote des concubins (art. 197, I, 4, a)** : deux concubins ont chacun la
  décote de 897 € ; un couple marié ou pacsé n'a que 1 483 € (< 1 794 €). Deux
  salaires de 19 500 € : **418 €/an** en mariage, **0 €** en concubinage — un PACS
  dissous rapporte autant. *Correction :* porter la décote du couple à 2 × 897 €
  (ou la calculer par membre du couple).
* **M3 — seuils franchis avec 1 € de déduction** : 1 € versé sur un PER supprime
  **62 €** d'impôt au seuil de recouvrement de 61 € (art. 1657, 1 bis) et rend à un
  retraité l'abattement de 1 411 € de l'art. 157 bis, soit **226 €** économisés.
  *Correction :* lisser ces seuils (dégressivité) au lieu de les appliquer en tout
  ou rien.

### Effets de seuil défavorables (l'inverse d'une faille)

`cgi-scan` balaie le revenu euro par euro ([`docs/falaises.md`](docs/falaises.md)) :
1 € de pension en plus coûte **226 €** à un retraité seul à 31 590 € de pension (perte
de l'abattement de l'art. 157 bis) ; le seuil de recouvrement crée partout un saut de
61 € ; dans la zone de décote de la CDHR, le taux marginal atteint **82,5 %**.

---

## 2. Le modèle Lean de l'impôt sur les revenus de 2025

`lean/CGI/IR2025/` : barème (art. 197, I, 1), quotient familial et plafonnement
(art. 194, 195, 197, I, 2), décote (197, I, 4), revenus catégoriels, charges et
abattements, réductions et crédits courants, prélèvement de 12,8 % (200 A), CEHR,
CDHR, revenu fiscal de référence, seuils de 61 €, 12 € et 8 €. Théorèmes prouvés
(sans `sorry`) : monotonie du barème, effet des parts, plafond de 45 %, absence de
« trou » après décote, taux marginal implicite de la décote, etc.

* **Brochure pratique 2026** : les **3 671 montants imprimés** du barème (toutes
  situations, 1 à 5 parts) sont reproduits exactement. La brochure contient une
  erreur : le tableau 7 (seuils de mise en recouvrement) majore de 9 € tous les seuils
  des couples (elle a appliqué « > 61 € » au lieu de « ≥ 61 € ») et imprime 62 869 €
  au lieu de 61 860 € pour 4,5 parts.
* **OpenFisca-France** (5 000 foyers, `cgi-households` / `cgi-compare`) : 249 foyers
  en désaccord, **tous attribués** — cinq défauts d'OpenFisca (abattement 157 bis jamais
  doublé : addition de booléens numpy ; réduction de 2 011 € des veufs ignorée en
  résidence alternée ; majoration « plus de 65 ans » de l'emploi à domicile absente ;
  dons 7UD plafonnés à 2 000 € avant le 14 octobre 2025 ; signe inversé dans
  `nb_pac2`), une lecture divergente de l'art. 1657, 1 bis et un arrondi. La comparaison
  a aussi **corrigé quatre fois le modèle Lean** (frais réels, retraitement de la CDHR,
  plafond du parent isolé en résidence alternée, seuils de 12 € et 8 €).

## 3. Le texte du CGI

`cgi-legi` extrait en une passe (≈ 4 min) les 38 237 versions d'articles du CGI, de
ses annexes et du LPF, plus un index de tous les codes et des lois. `cgi-refs`
contrôle les renvois, subdivisions, parenthèses et numérotations ; `cgi-triage` et
`cgi-sample` pré-classent et échantillonnent. Défauts confirmés à la relecture,
par exemple : l'annexe III (art. 350 quater) cite cinq articles abrogés lors de la
recodification des accises en 2022 ; le LPF (art. R*211-1) renvoie à l'art. 1414
abrogé en 2023 ; l'annexe II (371 Z quindecies) et l'annexe IV (164 F quatervicies)
citent des articles abrogés en 2016 ; l'annexe III cite le « III de l'article 302 M »
qui n'a plus que deux subdivisions. La précision mesurée sur échantillon varie selon
le contrôle (≈ 80 % pour les renvois à des articles abrogés, faible pour les autres) :
ces constats bruts restent à relire.

## Reproduire

```sh
curl -O https://echanges.dila.gouv.fr/OPENDATA/LEGI/Freemium_legi_global_<date>.tar.gz
(cd tools && cargo build --release)
./tools/target/release/cgi-legi Freemium_legi_global_<date>.tar.gz data
./tools/target/release/cgi-refs --data data --out data/findings.jsonl
(cd lean && lake build)                       # modèle, preuves et tests de la brochure
./tools/target/release/cgi-failles            # failles  → search/out/failles.md
./tools/target/release/cgi-scan               # falaises → search/out/falaises.md
./tools/target/release/cgi-households > foyers.jsonl
./lean/.lake/build/bin/ir2025 < foyers.jsonl > lean.jsonl
python validation/openfisca_run.py < foyers.jsonl > of.jsonl   # pip install openfisca-france
./tools/target/release/cgi-compare foyers.jsonl lean.jsonl of.jsonl
```

## Limites

Pas de relecture par un juriste. Le modèle couvre l'essentiel de l'impôt des ménages
mais pas les déficits, le système du quotient au barème, les revenus étrangers, les
réductions d'investissement, le lissage de la CEHR ni les prélèvements sociaux. Les
failles F1 et F2 sont mesurées sur une seule année, à droit constant ; F1 suppose
que le gain remplit les conditions du 9ᵉ alinéa du II de l'art. 224. Un accord entre
modèles n'est pas une preuve de vérité ; les théorèmes et les montants officiels
sont les vrais garde-fous.
