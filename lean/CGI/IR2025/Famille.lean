import CGI.IR2025.Bareme

/-!
# Quotient familial — CGI, art. 194, 195 et 197, I, 2 (revenus 2025)

* art. 194, I : nombre de parts selon la situation et les personnes à charge
  (½ part pour chacune des deux premières, 1 part à compter de la troisième ;
  les enfants en résidence alternée ouvrent droit à la moitié de ces
  majorations) ; le veuf ayant des personnes à charge est traité comme un
  couple marié ;
* art. 194, II : ½ part supplémentaire (case T) pour le parent célibataire ou
  divorcé vivant seul (¼ par enfant, pour les deux premiers, s'il n'a que des
  enfants en résidence alternée) ;
* art. 195, 1 : 1,5 part pour la personne seule sans charge de famille dans
  les cas a (case L : a élevé seule un enfant pendant cinq ans), c et d
  (invalidité, case P ; ancien combattant de plus de 74 ans, case W) et f
  (veuve de guerre, case G) ;
* art. 195, 2 à 6 : ½ part par personne à charge invalide, par conjoint
  invalide, pour l'ancien combattant de plus de 74 ans marié (case S) ;
* art. 197, I, 2 : plafonnement des effets du quotient familial — 1 807 € par
  demi-part (la moitié par quart de part) ; 4 262 € pour les deux premières
  demi-parts du parent isolé ; 1 079 € pour la demi-part de la case L ;
  réductions complémentaires de 1 801 € (invalides, anciens combattants,
  veuves de guerre) et de 2 011 € (veufs ayant des personnes à charge).

Les montants de plafond sont tenus en **demi-euros** (`× 2`) pour rester
exacts lorsque des quarts de part (903,50 €) interviennent.
-/

namespace CGI.IR2025

inductive Situation where
  | celibataire | marie | pacse | divorce | veuf
  deriving Repr, DecidableEq, BEq, Inhabited

def Situation.couple : Situation → Bool
  | .marie | .pacse => true
  | _ => false

/-- Éléments du cadre A et C de la déclaration 2042. -/
structure Famille where
  situation : Situation := .celibataire
  caseP : Bool := false   -- déclarant 1 titulaire d'une carte d'invalidité
  caseF : Bool := false   -- déclarant 2 titulaire d'une carte d'invalidité
  caseW : Bool := false   -- personne seule, plus de 74 ans, carte du combattant
  caseS : Bool := false   -- couple : l'un a plus de 74 ans et la carte du combattant
  caseG : Bool := false   -- pension de veuve de guerre
  caseL : Bool := false   -- vit seul, a élevé seul un enfant pendant 5 ans
  caseT : Bool := false   -- parent isolé (célibataire ou divorcé vivant seul)
  nbF : Nat := 0          -- enfants à charge exclusive ou principale
  nbG : Nat := 0          -- dont titulaires de la carte d'invalidité
  nbJ : Nat := 0          -- enfants majeurs célibataires rattachés
  nbH : Nat := 0          -- enfants en résidence alternée
  nbI : Nat := 0          -- dont titulaires de la carte d'invalidité
  nbR : Nat := 0          -- personnes invalides recueillies
  deriving Repr, Inhabited

namespace Famille

def b (x : Bool) : Int := if x then 1 else 0

/-- Personnes à charge à titre exclusif ou principal (art. 196 et 196 A bis). -/
def pac (f : Famille) : Int := f.nbF + f.nbJ + f.nbR

/-- Le veuf ayant au moins une personne à charge suit le régime des mariés. -/
def veufAvecCharge (f : Famille) : Bool :=
  f.situation == .veuf && (f.nbF + f.nbJ + f.nbH + f.nbR > 0)

/-- Parts « de base » (art. 194, I) : 2 pour un couple ou un veuf avec charge. -/
def base4 (f : Famille) : Int :=
  if f.situation.couple || f.veufAvecCharge then 8 else 4

/-- Base retenue pour le plafonnement (art. 197, I, 2) : 2 parts pour les
couples, 1 part pour toutes les autres situations, y compris les veufs. -/
def basePlaf4 (f : Famille) : Int := if f.situation.couple then 8 else 4

/-- Majoration pour personnes à charge exclusive (en quarts). -/
def majPac4 (f : Famille) : Int := 4 * f.pac - 2 * min f.pac 2

/-- Majoration pour enfants en résidence alternée (en quarts) : moitié de la
majoration correspondant à leur rang, les enfants à charge exclusive étant
comptés en premier (art. 194, I, 4ᵉ à 6ᵉ alinéas). -/
def majAlt4 (f : Famille) : Int :=
  let h : Int := f.nbH
  if f.pac = 0 then (if h ≥ 1 then 1 else 0) + (if h ≥ 2 then 1 else 0) + 2 * pos (h - 2)
  else if f.pac = 1 then (if h ≥ 1 then 1 else 0) + 2 * pos (h - 1)
  else 2 * h

/-- Demi-parts d'invalidité (art. 195, 2 à 5) : déclarants, enfants à charge,
personnes recueillies ; quart de part pour l'enfant alterné invalide. -/
def majInv4 (f : Famille) : Int :=
  2 * (b f.caseP + b f.caseF + f.nbG + f.nbR) + f.nbI

/-- Ancien combattant marié de plus de 74 ans (art. 195, 6), si aucun des
conjoints n'est déjà invalide. -/
def majCombattantCouple4 (f : Famille) : Int :=
  if f.situation.couple && f.caseS && !(f.caseP || f.caseF) then 2 else 0

/-- Case T (art. 194, II) : ½ part en présence d'une personne à charge
exclusive ; sinon ¼ (un enfant alterné) ou ½ (deux ou plus). -/
def majT4 (f : Famille) : Int :=
  if !f.caseT || f.base4 = 8 || !(f.situation == .celibataire || f.situation == .divorce) then 0
  else if f.pac > 0 then 2
  else if f.nbH = 1 then 1
  else if f.nbH ≥ 2 then 2
  else 0

/-- ½ part de l'art. 195, 1 (cases L, W, G) pour la personne seule sans
personne à charge, non cumulable avec celle de l'invalidité (case P). -/
def maj195 (f : Famille) : Bool :=
  f.base4 = 4 && f.pac = 0 && f.nbH = 0 && !f.caseP && (f.caseL || f.caseW || f.caseG)

def maj195_4 (f : Famille) : Int := if f.maj195 then 2 else 0

/-- Nombre de parts, en quarts. -/
def parts4 (f : Famille) : Int :=
  f.base4 + f.majPac4 + f.majAlt4 + f.majInv4 + f.majCombattantCouple4 + f.majT4 + f.maj195_4

/-! ### Plafonnement (art. 197, I, 2), en demi-euros -/

def plafDemiPart : Int := 1807
def plafParentIsole : Int := 4262
def plafCaseL : Int := 1079
def reducComplInvalide : Int := 1801
def reducComplVeuf : Int := 2011

/-- Quarts « alternés » plafonnés au taux majoré du parent isolé : les deux
quarts accordés pour chacun des deux premiers enfants alternés (enfant + case
T) lorsque le parent isolé n'a aucune personne à charge exclusive. -/
def quartsParentIsoleAlt (f : Famille) : Int :=
  if f.majT4 > 0 && f.pac = 0 then 2 * min (f.nbH : Int) 2 else 0

/-- 2 × plafond global des avantages procurés par les parts excédant la base. -/
def plafond2 (f : Famille) : Int :=
  let excedent4 := f.parts4 - f.basePlaf4
  let piAlt := f.quartsParentIsoleAlt
  -- quarts plafonnés à 1 807 / 2 chacun (soit 1 807 en demi-euros)
  let ordinaires := excedent4 - piAlt
  let base := ordinaires * plafDemiPart + piAlt * plafParentIsole
  -- parent isolé avec charge exclusive : 4 262 € au lieu de 2 × 1 807 € pour
  -- les deux premières demi-parts
  let bonusT := if f.majT4 > 0 && f.nbF + f.nbJ > 0 then 2 * (plafParentIsole - 2 * plafDemiPart) else 0
  -- case L seule : 1 079 € au lieu de 1 807 € pour la demi-part
  let malusL := if f.maj195 && f.caseL && !(f.caseW || f.caseG) then 2 * (plafDemiPart - plafCaseL) else 0
  base + bonusT - malusL

/-- 2 × réductions d'impôt complémentaires (art. 197, I, 2, 3ᵉ et 4ᵉ alinéas). -/
def complement2 (f : Famille) : Int :=
  let veuf := if f.veufAvecCharge then 2 * reducComplVeuf else 0
  let demiParts := b f.caseP + b f.caseF + f.nbG + f.nbR
                    + (if f.majCombattantCouple4 > 0 then 1 else 0)
                    + (if f.maj195 && (f.caseW || f.caseG) then 1 else 0)
  veuf + 2 * reducComplInvalide * demiParts + reducComplInvalide * f.nbI

end Famille

/-- Impôt après plafonnement des effets du quotient familial :
`max(I, A − plafond − compléments)` où `I` est calculé avec toutes les parts
et `A` avec les parts de base (1 ou 2).  Les montants en demi-euros sont
ramenés à l'euro par arrondi au plus proche. -/
def impotPlafonneQ (r q qBase plaf2 compl2 : Int) : Int :=
  let i := droitsSimples r q
  let a := droitsSimples r qBase
  max i (arrondi (2 * a - plaf2 - compl2) 2)

def impotPlafonne (r : Int) (f : Famille) : Int :=
  impotPlafonneQ r f.parts4 f.basePlaf4 f.plafond2 f.complement2

/-! ## Décote — art. 197, I, 4, a -/

def seuilDecoteSeul : Int := 897
def seuilDecoteCouple : Int := 1483

/-- Décote : « différence entre 897 € (1 483 € pour les contribuables soumis
à imposition commune) et les 45,25 % de son montant », limitée à l'impôt. -/
def decote (couple : Bool) (impot : Int) : Int :=
  let s := if couple then seuilDecoteCouple else seuilDecoteSeul
  min (pos (arrondi (s * 10000 - impot * 4525) 10000)) (pos impot)

theorem decote_le (c : Bool) (i : Int) : decote c i ≤ pos i := by
  cases c <;> simp only [decote, pos] <;> omega

theorem decote_nonneg (c : Bool) (i : Int) : 0 ≤ decote c i := by
  cases c <;> simp only [decote, pos] <;> omega

/-- L'impôt après décote est croissant : la décote ne crée jamais de « trou ». -/
theorem apres_decote_mono (c : Bool) {i j : Int} (h : i ≤ j) (hi : 0 ≤ i) :
    i - decote c i ≤ j - decote c j := by
  cases c <;> simp only [decote, arrondi, pos, seuilDecoteSeul, seuilDecoteCouple] <;> omega

/-- Au-delà de 1 982 € (personne seule) l'impôt n'est plus décoté. -/
theorem decote_nulle_seul {i : Int} (h : 1982 ≤ i) : decote false i = 0 := by
  unfold decote arrondi pos seuilDecoteSeul seuilDecoteCouple; simp; omega

theorem decote_nulle_couple {i : Int} (h : 3277 ≤ i) : decote true i = 0 := by
  unfold decote arrondi pos seuilDecoteSeul seuilDecoteCouple; simp; omega

/-- En dessous de 618 €, la décote absorbe tout l'impôt d'une personne seule. -/
theorem decote_totale_seul {i : Int} (h0 : 0 ≤ i) (h : i ≤ 617) :
    i - decote false i = 0 := by
  unfold decote arrondi pos seuilDecoteSeul seuilDecoteCouple; simp; omega

/-- Entre 618 € et 1 982 €, 100 € d'impôt brut supplémentaires augmentent
l'impôt dû de 144 à 146 € : la décote porte le taux marginal effectif de la
tranche à 11 % à environ 16 % (11 % × 1,4525). -/
theorem decote_pente (i : Int) (h0 : 618 ≤ i) (h : i + 100 ≤ 1982) :
    144 ≤ (i + 100 - decote false (i + 100)) - (i - decote false i) ∧
    (i + 100 - decote false (i + 100)) - (i - decote false i) ≤ 146 := by
  unfold decote arrondi pos seuilDecoteSeul seuilDecoteCouple; simp; omega

end CGI.IR2025
