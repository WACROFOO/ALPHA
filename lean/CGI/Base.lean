/-!
# Arithmétique monétaire

Tous les montants sont des euros entiers (`Int`).  Les taux sont représentés
par des fractions exactes : aucune virgule flottante n'intervient.

L'arrondi suit le CGI, art. 193 et 1657, 1 : « les fractions d'euro
inférieures à 0,50 sont négligées et celles égales ou supérieures à 0,50 sont
comptées pour 1 ».
-/

namespace CGI

/-- Arrondi à l'euro le plus proche de `num / den` (`den > 0`) ; 0,50 compte pour 1. -/
def arrondi (num den : Int) : Int := (2 * num + den) / (2 * den)

/-- Partie positive. -/
def pos (x : Int) : Int := max x 0

/-- `min` borné à zéro : `plafonner x p = min (max x 0) p`. -/
def plafonner (x p : Int) : Int := min (pos x) p

theorem arrondi_mono {a b den : Int} (h : a ≤ b) (hd : 0 < den) :
    arrondi a den ≤ arrondi b den := by
  unfold arrondi
  exact Int.ediv_le_ediv (by omega) (by omega)

theorem arrondi_entier (a : Int) : arrondi (a * 400) 400 = a := by
  unfold arrondi; omega

theorem pos_nonneg (x : Int) : 0 ≤ pos x := by unfold pos; omega

end CGI
