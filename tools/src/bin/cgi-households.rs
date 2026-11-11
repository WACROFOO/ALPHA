//! `cgi-households [--seed N] [--ordinaires 2000] [--difficiles 3000]` — foyers en JSONL.
use std::io::Write;

fn main() -> std::io::Result<()> {
    let (mut seed, mut n_ord, mut n_diff) = (2025u64, 2000usize, 3000usize);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let v = args.next().expect("valeur manquante");
        match a.as_str() {
            "--seed" => seed = v.parse().unwrap(),
            "--ordinaires" => n_ord = v.parse().unwrap(),
            "--difficiles" => n_diff = v.parse().unwrap(),
            _ => panic!("argument inconnu : {a}"),
        }
    }
    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    for h in cgi_tools::menages::echantillon(seed, n_ord, n_diff) {
        serde_json::to_writer(&mut out, &h)?;
        out.write_all(b"\n")?;
    }
    Ok(())
}
