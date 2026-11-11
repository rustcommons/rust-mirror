mod signals;
fn main() {
    let cancel = rust_mirror::events::CancelToken::new();
    if let Err(error) = signals::install(cancel.clone()) {
        eprintln!("rust-mirror: signal setup failed: {error}");
        std::process::exit(1);
    }
    let code = rust_mirror::cli::execute(std::env::args_os().skip(1).collect(), cancel);
    std::process::exit(code);
}
