use cargo_member::cli::{Cargo, Context};
use structopt::StructOpt as _;

fn main() {
    let Cargo::Member(opt) = Cargo::from_args();
    let color = opt.color();
    cargo_member::cli::init_logger(color);
    let mut stderr = cargo_member::cli::stderr(color);
    if let Err(err) = Context::new(&mut stderr).and_then(|ctx| cargo_member::cli::run(opt, ctx)) {
        cargo_member::cli::exit_with_error(err, color);
    }
}
