#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let origin = if args
        .get(1)
        .is_some_and(|id| id == rusty_vault::browser::FIREFOX_EXTENSION_ID)
    {
        &args[1]
    } else {
        args.first().map(String::as_str).unwrap_or_default()
    };
    if rusty_vault::browser::native_host(
        origin,
        &mut std::io::stdin().lock(),
        &mut std::io::stdout().lock(),
    )
    .is_err()
    {
        std::process::exit(1);
    }
}
