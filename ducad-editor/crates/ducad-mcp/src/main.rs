//! `ducad-mcp [--root DIR]` — server Model Context Protocol lewat stdio.
//! stdout hanya berisi pesan protokol; log ke stderr.

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .target(env_logger::Target::Stderr)
        .init();
    let mut args = std::env::args().skip(1);
    let mut root = std::env::current_dir()?;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--root" => {
                let dir = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("--root butuh direktori"))?;
                root = PathBuf::from(dir);
            }
            "--help" | "-h" => {
                eprintln!("pemakaian: ducad-mcp [--root DIR]");
                return Ok(());
            }
            other => anyhow::bail!("argumen tidak dikenal: {other}"),
        }
    }
    let mut server = ducad_mcp::server::Server::new(root)?;
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    ducad_mcp::server::serve(&mut server, stdin.lock(), stdout.lock())
}
