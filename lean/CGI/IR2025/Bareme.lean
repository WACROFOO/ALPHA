import CGI.Base

/-!
# Barème progressif — CGI, art. 197, I, 1 (revenus 2025)

Texte (version issue de l'article 4 de la loi n° 2026-103 du 19 février 2026
de finances pour 2026) :

> « L'impôt est calculé en appliquant à la fraction de chaque part de revenu
> qui excède 11 600 € le taux de : 11 % pour la fraction supérieure à 11 600 €
> et inférieure ou égale à 29 579 € ; 30 % pour la fraction supérieure à
> 29 579 € et inférieure ou égale à 84 577 € ; 41 % pour la fraction
> supérieure à 84 577 € et inférieure ou égale à 181 917 € ; 45 % pour la
> fraction supérieure à 181 917 €. »

Le nombre de parts est représenté en **quarts de part** (`q = 4 × N`), ce
qui couvre les quarts de part des enfants en résidence alternée.  Pour un
revenu imposable `R` et `N = q / 4` parts, l'impôt est
`N × Σ tᵢ × (min(R/N, sᵢ₊₁) − sᵢ)⁺`, soit, avec les taux marginaux
différentiels `Δtᵢ` : `Σ Δtᵢ × (R − N·sᵢ)⁺`.  Multiplié par 400 (4 pour les
quarts × 100 pour les pourcentages) ce montant est un entier exact.
-/

namespace CGI.IR2025

/-- Seuils des tranches, par part (art. 197, I, 1). -/
def seuil1 : Int := 11600
def seuil2 : Int := 29579
def seuil3 : Int := 84577
def seuil4 : Int := 181917

/-- 400 × impôt brut, exact (taux différentiels 11, 19, 11 et 4 points). -/
def bareme400 (r q : Int) : Int :=
  11 * pos (4 * r - q * seuil1) + 19 * pos (4 * r - q * seuil2)
    + 11 * pos (4 * r - q * seuil3) + 4 * pos (4 * r - q * seuil4)

/-- Droits simples résultant du barème, arrondis à l'euro le plus proche. -/
def droitsSimples (r q : Int) : Int := arrondi (bareme400 r q) 400

/-- Taux marginal (en %) applicable à un revenu par part. -/
def tauxMarginal (r q : Int) : Int :=
  if 4 * r > q * seuil4 then 45
  else if 4 * r > q * seuil3 then 41
  else if 4 * r > q * seuil2 then 30
  else if 4 * r > q * seuil1 then 11
  else 0

/-! ## Propriétés -/

theorem bareme400_nonneg (r q : Int) : 0 ≤ bareme400 r q := by
  unfold bareme400 pos; omega

/-- Le barème est croissant en fonction du revenu, à nombre de parts fixé. -/
theorem bareme400_mono {r r' : Int} (q : Int) (h : r ≤ r') :
    bareme400 r q ≤ bareme400 r' q := by
  unfold bareme400 pos; omega

/-- Ajouter des parts ne peut qu'abaisser l'impôt (q ≥ 0). -/
theorem bareme400_antitone_parts (r : Int) {q q' : Int} (h : q ≤ q') :
    bareme400 r q' ≤ bareme400 r q := by
  unfold bareme400 pos seuil1 seuil2 seuil3 seuil4; omega

/-- Le taux moyen n'excède jamais le taux marginal supérieur de 45 %. -/
theorem bareme400_le_45 (r q : Int) (hq : 0 ≤ q) (hr : 0 ≤ r) :
    bareme400 r q ≤ 45 * 4 * r := by
  unfold bareme400 pos seuil1 seuil2 seuil3 seuil4; omega

/-- Un euro de revenu supplémentaire coûte au plus 45 centimes (avant arrondi). -/
theorem bareme400_lipschitz (r q : Int) :
    bareme400 (r + 1) q ≤ bareme400 r q + 45 * 4 := by
  unfold bareme400 pos; omega

/-- Aucun impôt tant que le revenu par part n'excède pas 11 600 €. -/
theorem bareme400_zero {r q : Int} (h : 4 * r ≤ q * seuil1) (hq : 0 ≤ q) :
    bareme400 r q = 0 := by
  unfold bareme400 pos seuil1 seuil2 seuil3 seuil4 at *; omega

theorem droitsSimples_mono {r r' : Int} (q : Int) (h : r ≤ r') :
    droitsSimples r q ≤ droitsSimples r' q :=
  arrondi_mono (bareme400_mono q h) (by decide)

theorem droitsSimples_nonneg (r q : Int) : 0 ≤ droitsSimples r q := by
  have := bareme400_nonneg r q
  unfold droitsSimples arrondi; omega

/-- Forme « R × taux − N × constante » de la brochure (tableau 5) : dans la
tranche à 30 %, `impôt = 0,30 R − 6 896,01 N`, soit `400 × impôt = 120 R − 689 601 q`
(avec `q = 4N` : 689 601 / 400 × 4 = 6 896,01). -/
theorem bareme400_tranche30 {r q : Int} (hq : 0 ≤ q)
    (h1 : q * seuil2 ≤ 4 * r) (h2 : 4 * r ≤ q * seuil3) :
    bareme400 r q = 120 * r - 689601 * q := by
  simp only [seuil1, seuil2, seuil3, seuil4] at h1 h2
  have e1 : pos (4 * r - q * seuil1) = 4 * r - q * 11600 := by
    simp only [pos, seuil1]; omega
  have e2 : pos (4 * r - q * seuil2) = 4 * r - q * 29579 := by
    simp only [pos, seuil2]; omega
  have e3 : pos (4 * r - q * seuil3) = 0 := by simp only [pos, seuil3]; omega
  have e4 : pos (4 * r - q * seuil4) = 0 := by simp only [pos, seuil4]; omega
  unfold bareme400; rw [e1, e2, e3, e4]; omega

end CGI.IR2025
