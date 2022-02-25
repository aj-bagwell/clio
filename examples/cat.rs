use clap::Parser;
use clio::*;

#[derive(Parser)]
#[clap(name = "cat")]
struct Opt {
    /// Input file, use '-' for stdin
    #[clap(parse(try_from_os_str = Input::try_from_os_str), default_value="-")]
    input: Input,

    /// Output file '-' for stdout
    #[clap(long, short, parse(try_from_os_str = Output::try_from_os_str), default_value="-")]
    output: Output,
}

fn main() {
    let mut opt = Opt::parse();

    std::io::copy(&mut opt.input, &mut opt.output).unwrap();
}
