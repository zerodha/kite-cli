use clap::Parser;
use kite::{app, cli, commands};

#[tokio::main]
async fn main() {
    let cli = cli::Cli::parse();
    let json_mode = cli.json;
    let mut failed_check = false;
    if let Err(err) = commands::run(cli).await.and_then(|output| {
        failed_check = !output.passed();
        app::output::print(output, json_mode)
    }) {
        if err
            .downcast_ref::<std::io::Error>()
            .is_some_and(|error| error.kind() == std::io::ErrorKind::BrokenPipe)
        {
            return;
        }
        app::output::print_error(&err, json_mode);
        std::process::exit(1);
    }
    if failed_check {
        std::process::exit(1);
    }
}
