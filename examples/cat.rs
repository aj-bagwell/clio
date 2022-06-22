use clap::Parser;
use clio::*;

#[derive(Parser)]
#[clap(name = "cat")]
struct Opt {
    /// Input file, use '-' for stdin
    #[clap(value_parser, default_value = "-")]
    input: Input,

    /// Output file '-' for stdout
    #[clap(long, short, value_parser, default_value = "-")]
    output: Output,
}

fn main() {
    let mut opt = Opt::parse();

    std::io::copy(&mut opt.input, &mut opt.output).unwrap();
}
