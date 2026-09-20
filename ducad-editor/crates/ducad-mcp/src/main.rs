//! `ducad-mcp [--root DIR]` — server Model Context Protocol lewat stdio.
//! stdout hanya berisi pesan protokol; log ke stderr.

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .target(env_logger::Target::Stderr)
        .init();
    let mut args = std::env::args().skip(1);
    let mut root = std::env::current_dir()?;
    let mut attach = false;
    let mut socket: Option<PathBuf> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--root" => {
                let dir = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("--root butuh direktori"))?;
                root = PathBuf::from(dir);
            }
            "--attach" => attach = true,
            "--socket" => {
                let p = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("--socket butuh path"))?;
                socket = Some(PathBuf::from(p));
                attach = true;
            }
            "--help" | "-h" => {
                eprintln!(
                    "pemakaian: ducad-mcp [--root DIR] [--attach [--socket PATH]]\n\n  \
                     --attach  teruskan semua tool ke aplikasi DUCAD yang sedang terbuka\n            \
                     (Settings → Agent Bridge). Default soket: $HOME/.ducad/agent.sock"
                );
                return Ok(());
            }
            other => anyhow::bail!("argumen tidak dikenal: {other}"),
        }
    }
    let mut server = if attach {
        let socket = socket.unwrap_or_else(ducad_mcp::attach::default_socket);
        ducad_mcp::server::Server::attached(root, socket)?
    } else {
        ducad_mcp::server::Server::new(root)?
    };
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    ducad_mcp::server::serve(&mut server, stdin.lock(), stdout.lock())
}
