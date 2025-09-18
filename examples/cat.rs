use clio::*;

#[cfg(feature = "clap-parse")]
use clap::Parser;
#[cfg(feature = "clap-parse")]
#[derive(Parser)]
#[clap(name = "cat")]
struct Opt {
    /// Input file, use '-' for stdin
    #[arg(default_values_t = vec![Input::std()])]
    inputs: Vec<Input>,

    /// Output file '-' for stdout
    #[clap(long, short, value_parser, default_value = "-")]
    output: Output,
}

#[cfg(feature = "clap-parse")]
fn main() {
    let mut opt = Opt::parse();

    for mut input in opt.inputs {
        std::io::copy(&mut input, &mut opt.output).unwrap();
    }
}

#[cfg(not(feature = "clap-parse"))]
fn main() {
    for arg in std::env::args_os().skip(1) {
        let mut input = Input::new(&arg).unwrap();
        std::io::copy(&mut input, &mut Output::std()).unwrap();
    }
}
