//! Outils pour la formalisation du Code général des impôts.
//!
//! * [`legi`] : extraction et analyse du flux open data LEGI de la DILA ;
//! * [`refs`] : contrôles de rédaction (renvois, subdivisions, parenthèses…) ;
//! * [`util`] : découpage de chaînes UTF-8 en caractères.

pub mod legi;
pub mod refs;
pub mod util;
pub mod menages;
