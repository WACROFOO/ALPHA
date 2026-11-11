//! `cgi-legi <Freemium_legi_global.tar.gz> [data]` — une passe sur l'archive LEGI.
use std::path::PathBuf;

fn main() -> std::io::Result<()> {
    let mut args = std::env::args().skip(1);
    let tar = PathBuf::from(args.next().expect("usage: cgi-legi <archive.tar.gz> [répertoire de sortie]"));
    let out = PathBuf::from(args.next().unwrap_or_else(|| "data".into()));
    cgi_tools::legi::extract(&tar, &out)?;
    Ok(())
}
