import CGI.IR2025.Famille

/-!
# Le foyer fiscal et le calcul complet de l'impôt (revenus 2025)

Chaîne de calcul (CGI, art. 156, 193, 197 et suivants) :

1. revenus catégoriels nets → revenu brut global (art. 156) ;
2. charges déductibles → revenu net global ;
3. abattements spéciaux (art. 157 bis, 196 B) → revenu net imposable ;
4. barème et quotient familial (art. 193, 194, 197, I, 1 et 2) ;
5. décote (art. 197, I, 4, a) ;
6. réductions d'impôt (imputées sur l'impôt au barème, art. 193) ;
7. impôt au taux forfaitaire de 12,8 % (art. 200 A) ;
8. crédits d'impôt (restituables) ; seuil de recouvrement de 61 € (art. 1657, 1 bis) ;
9. contributions sur les hauts revenus : CEHR (art. 223 sexies) et CDHR (art. 224).

Les cases de la déclaration 2042 correspondantes sont indiquées en commentaire.
-/

namespace CGI.IR2025

/-- Revenus et charges propres à un déclarant. -/
structure Personne where
  salaires : Int := 0          -- 1AJ / 1BJ
  fraisReels : Int := 0        -- 1AK / 1BK (0 : déduction forfaitaire de 10 %)
  pensions : Int := 0          -- 1AS / 1BS
  microBicVente : Int := 0     -- 5KO / 5LO (ventes, abattement 71 %)
  microBicServices : Int := 0  -- 5KP / 5LP (prestations, abattement 50 %)
  microBnc : Int := 0          -- 5HQ / 5IQ (abattement 34 %)
  plus65 : Bool := false       -- né avant le 1ᵉʳ janvier 1961
  versementsPER : Int := 0     -- 6NS / 6NT
  plafondPER : Int := 0        -- plafond de déduction disponible (avis 2025)
  cotisationsSyndicales : Int := 0 -- 7AC / 7AE
  deriving Repr, Inhabited

structure Foyer where
  famille : Famille := {}
  d1 : Personne := {}
  d2 : Personne := {}
  dividendes : Int := 0        -- 2DC
  interets : Int := 0          -- 2TR
  plusValues : Int := 0        -- 3VG
  optionBareme : Bool := false -- 2OP
  microFoncier : Int := 0      -- 4BE (recettes brutes)
  csgDeductible : Int := 0     -- 6DE
  pensionsEnfantsMajeurs : List Int := []  -- 6EL / 6EM, par enfant
  autresPensions : Int := 0    -- 6GU
  foyersRattaches : Nat := 0   -- enfants mariés ou chargés de famille rattachés
  dons75 : Int := 0            -- 7UD (aide aux personnes en difficulté)
  dons66 : Int := 0            -- 7UF (autres organismes d'intérêt général)
  emploiDomicile : Int := 0    -- 7DB
  premiereAnneeEmploi : Bool := false -- 7DQ
  gardeEnfants : List Int := [] -- 7GA à 7GC, par enfant de moins de 6 ans à charge exclusive
  scolCollege : Nat := 0       -- 7EA
  scolLycee : Nat := 0         -- 7EC
  scolSup : Nat := 0           -- 7EF
  deriving Repr, Inhabited

/-! ## Paramètres (revenus 2025) -/

namespace P
/-- art. 83, 3° : déduction forfaitaire pour frais professionnels. -/
def fpTaux : Int := 10
def fpMin : Int := 509
def fpMax : Int := 14555
/-- art. 158, 5, a : abattement sur les pensions. -/
def pensMin : Int := 454
def pensMax : Int := 4439
/-- art. 50-0 et 102 ter : abattements des régimes micro. -/
def microMin : Int := 305
/-- art. 156, II, 2° : pension alimentaire versée à un enfant majeur. -/
def pensionEnfantMax : Int := 6855
/-- art. 157 bis : abattement des personnes âgées ou invalides. -/
def abat157bisPlein : Int := 2822
def abat157bisDemi : Int := 1411
def abat157bisSeuil1 : Int := 17670
def abat157bisSeuil2 : Int := 28430
/-- art. 196 B : abattement par enfant marié rattaché. -/
def abat196B : Int := 6855
/-- art. 199 quater F : frais de scolarité. -/
def scolCollege : Int := 61
def scolLycee : Int := 153
def scolSup : Int := 183
/-- art. 200-0 A : plafonnement global des avantages fiscaux. -/
def plafondNiches : Int := 10000
/-- art. 1657, 1 bis : seuil de mise en recouvrement. -/
def seuilRecouvrement : Int := 61
end P

/-! ## 1. Revenus catégoriels -/

/-- Traitements et salaires nets (art. 83, 3°) : déduction de 10 % comprise
entre 509 € et 14 555 €, sans excéder le salaire ; ou frais réels. -/
def salairesNets (p : Personne) : Int :=
  let ded := if p.fraisReels > 0 then p.fraisReels
             else min (max (arrondi (p.salaires * P.fpTaux) 100) P.fpMin) P.fpMax
  pos (p.salaires - min ded (pos p.salaires))

/-- Abattement de 10 % sur les pensions d'un bénéficiaire, avec le minimum de
454 € (sans excéder la pension) — avant plafonnement global du foyer. -/
def abatPension (p : Personne) : Int :=
  min (max (arrondi (p.pensions * 10) 100) P.pensMin) (pos p.pensions)

/-- Pensions nettes du foyer : abattement total plafonné à 4 439 € (art. 158, 5, a). -/
def pensionsNettes (d1 d2 : Personne) : Int :=
  pos (d1.pensions + d2.pensions - min (abatPension d1 + abatPension d2) P.pensMax)

/-- Bénéfices micro (art. 50-0, 102 ter) : abattement forfaitaire, 305 € au moins. -/
def microNet (recettes taux : Int) : Int :=
  if recettes ≤ 0 then 0 else
  pos (recettes - min (max (arrondi (recettes * taux) 100) P.microMin) recettes)

def nonSalaries (p : Personne) : Int :=
  microNet p.microBicVente 71 + microNet p.microBicServices 50 + microNet p.microBnc 34

/-- Revenus de capitaux mobiliers imposés au barème sur option globale
(art. 200 A, 2) : abattement de 40 % sur les dividendes (art. 158, 3, 2°). -/
def rcmBareme (f : Foyer) : Int :=
  if f.optionBareme then f.dividendes - arrondi (f.dividendes * 40) 100 + f.interets + f.plusValues
  else 0

/-- Micro-foncier (art. 32) : abattement de 30 %. -/
def foncierNet (f : Foyer) : Int := f.microFoncier - arrondi (f.microFoncier * 30) 100

def revenuBrutGlobal (f : Foyer) : Int :=
  let couple := f.famille.situation.couple
  let d2 := if couple then f.d2 else {}
  salairesNets f.d1 + salairesNets d2 + pensionsNettes f.d1 d2
    + nonSalaries f.d1 + nonSalaries d2 + rcmBareme f + foncierNet f

/-! ## 2. Charges déductibles -/

def deductionPER (p : Personne) : Int := min (pos p.versementsPER) (pos p.plafondPER)

def pensionsDeductibles (f : Foyer) : Int :=
  (f.pensionsEnfantsMajeurs.map fun x => min (pos x) P.pensionEnfantMax).foldl (· + ·) 0
    + pos f.autresPensions

def charges (f : Foyer) : Int :=
  let d2 := if f.famille.situation.couple then f.d2 else {}
  pos f.csgDeductible + pensionsDeductibles f + deductionPER f.d1 + deductionPER d2

def revenuNetGlobal (f : Foyer) : Int := pos (revenuBrutGlobal f - charges f)

/-! ## 3. Abattements spéciaux -/

/-- art. 157 bis : par contribuable âgé de plus de 65 ans ou invalide. -/
def abattement157bis (f : Foyer) : Int :=
  let rng := revenuNetGlobal f
  let n : Int := (if f.d1.plus65 || f.famille.caseP then 1 else 0)
    + (if f.famille.situation.couple && (f.d2.plus65 || f.famille.caseF) then 1 else 0)
  let unit := if rng ≤ P.abat157bisSeuil1 then P.abat157bisPlein
              else if rng ≤ P.abat157bisSeuil2 then P.abat157bisDemi else 0
  n * unit

def abattementsSpeciaux (f : Foyer) : Int :=
  abattement157bis f + P.abat196B * f.foyersRattaches

def revenuImposable (f : Foyer) : Int := pos (revenuNetGlobal f - abattementsSpeciaux f)

/-! ## 6. Réductions et crédits d'impôt -/

/-- art. 200 : dons. 75 % jusqu'à 1 000 € pour les organismes d'aide aux
personnes en difficulté, l'excédent rejoignant les dons à 66 %, retenus dans
la limite de 20 % du revenu imposable. -/
def reductionDons (f : Foyer) (ri : Int) : Int :=
  let d75 := min (pos f.dons75) 1000
  let reste := pos f.dons75 - d75 + pos f.dons66
  arrondi (d75 * 75) 100 + arrondi (min reste (arrondi (ri * 20) 100) * 66) 100

def reductionScolarite (f : Foyer) : Int :=
  P.scolCollege * f.scolCollege + P.scolLycee * f.scolLycee + P.scolSup * f.scolSup

/-- art. 199 sexdecies : emploi d'un salarié à domicile — 50 % des dépenses
retenues dans la limite de 12 000 €, majorée de 1 500 € par enfant à charge
et par membre du foyer âgé de plus de 65 ans, sans excéder 15 000 €
(15 000 € et 18 000 € la première année). -/
def creditEmploiDomicile (f : Foyer) : Int :=
  let fam := f.famille
  let majorants : Int := fam.nbF + fam.nbJ + (if f.d1.plus65 then 1 else 0)
    + (if fam.situation.couple && f.d2.plus65 then 1 else 0)
  let base := if f.premiereAnneeEmploi then 15000 else 12000
  let maxi := if f.premiereAnneeEmploi then 18000 else 15000
  let plaf := min (base + 1500 * majorants + 750 * fam.nbH) maxi
  arrondi (min (pos f.emploiDomicile) plaf * 50) 100

/-- art. 200 quater B : frais de garde des enfants de moins de 6 ans — 50 %
des dépenses retenues dans la limite de 3 500 € par enfant. -/
def creditGarde (f : Foyer) : Int :=
  arrondi ((f.gardeEnfants.map fun x => min (pos x) 3500).foldl (· + ·) 0 * 50) 100

/-- art. 199 quater C : cotisations syndicales — 66 % dans la limite de 1 %
des salaires et pensions. -/
def creditSyndical (p : Personne) : Int :=
  let lim := arrondi ((pos p.salaires + pos p.pensions) * 1) 100
  arrondi (min (pos p.cotisationsSyndicales) lim * 66) 100

/-! ## 7. Impôt au taux forfaitaire — art. 200 A, 1 -/

def impotForfaitaire (f : Foyer) : Int :=
  if f.optionBareme then 0
  else arrondi ((pos f.dividendes + pos f.interets + pos f.plusValues) * 128) 1000

/-! ## Revenu fiscal de référence — art. 1417, IV -/

def revenuFiscalReference (f : Foyer) : Int :=
  let d2 := if f.famille.situation.couple then f.d2 else {}
  revenuImposable f
    + deductionPER f.d1 + deductionPER d2                         -- a du 1°
    + (if f.optionBareme then arrondi (f.dividendes * 40) 100      -- a bis du 1°
       else pos f.dividendes + pos f.interets + pos f.plusValues)  -- c du 1°

/-! ## 9. Contributions sur les hauts revenus -/

/-- art. 223 sexies : 3 % puis 4 % du RFR au-delà de 250 000 € / 500 000 €
(seuils doublés pour un couple soumis à imposition commune). -/
def cehr (couple : Bool) (rfr : Int) : Int :=
  let s := if couple then 500000 else 250000
  arrondi (3 * (min rfr (2 * s) - s |> pos) + 4 * pos (rfr - 2 * s)) 100

/-- art. 224 : contribution différentielle — taux minimal de 20 % sur le
revenu de référence, diminué de l'impôt et de la CEHR, majorés de 12 500 €
pour un couple et de 1 500 € par personne à charge ; décote de 82,5 % de
l'excédent sur 250 000 € (500 000 €) jusqu'à 330 000 € (660 000 €). -/
def cdhr (couple : Bool) (pac : Int) (base impots : Int) : Int :=
  let s := if couple then 500000 else 250000
  if base ≤ s then 0 else
  let c20 := arrondi (base * 20) 100
  let cot := min c20 (arrondi ((base - s) * 825) 1000)
  pos (cot - (impots + (if couple then 12500 else 0) + 1500 * pac))

/-! ## Résultat -/

structure Resultat where
  revenuBrutGlobal : Int
  charges : Int
  revenuNetGlobal : Int
  abattements : Int
  revenuImposable : Int
  parts4 : Int
  droitsSimples : Int
  impotPlafonne : Int
  decote : Int
  impotApresDecote : Int
  reductions : Int
  impotForfaitaire : Int
  impotAvantCredits : Int
  credits : Int
  cehr : Int
  cdhr : Int
  rfr : Int
  impotNet : Int      -- négatif : restitution
  deriving Repr, Inhabited

def calcul (f : Foyer) : Resultat :=
  let fam := f.famille
  let couple := fam.situation.couple
  let d2 := if couple then f.d2 else {}
  let rbg := revenuBrutGlobal f
  let ch := charges f
  let rng := revenuNetGlobal f
  let ab := abattementsSpeciaux f
  let ri := revenuImposable f
  let q := fam.parts4
  let ds := droitsSimples ri q
  let ip := impotPlafonne ri fam
  let dec := decote couple ip
  let iad := ip - dec
  -- réductions (non restituables), imputées sur l'impôt au barème
  let redBrut := reductionDons f ri + reductionScolarite f
  let red := min redBrut iad
  let ifo := impotForfaitaire f
  let iac := iad - red + ifo
  -- crédits d'impôt (restituables) ; plafonnement global des niches (art. 200-0 A)
  let nichesPlaf := min (creditEmploiDomicile f + creditGarde f) P.plafondNiches
  let cred := nichesPlaf + creditSyndical f.d1 + creditSyndical d2
  let rfr := revenuFiscalReference f
  let c1 := cehr couple rfr
  let c2 := cdhr couple fam.pac rfr (iac + c1)
  -- art. 1657, 1 bis : cotisation inférieure à 61 € non mise en recouvrement
  let iacRec := if iac + c1 + c2 < P.seuilRecouvrement then 0 else iac + c1 + c2
  { revenuBrutGlobal := rbg, charges := ch, revenuNetGlobal := rng, abattements := ab,
    revenuImposable := ri, parts4 := q, droitsSimples := ds, impotPlafonne := ip,
    decote := dec, impotApresDecote := iad, reductions := red, impotForfaitaire := ifo,
    impotAvantCredits := iac, credits := cred, cehr := c1, cdhr := c2, rfr := rfr,
    impotNet := iacRec - cred }

end CGI.IR2025
