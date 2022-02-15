use clio::*;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(name = "cat")]
struct Opt {
    /// Input file, use '-' for stdin
    #[structopt(parse(try_from_os_str = Input::try_from_os_str), default_value="-")]
    input: Input,

    /// Output file '-' for stdout
    #[structopt(long, short, parse(try_from_os_str = Output::try_from_os_str), default_value="-")]
    output: Output,
}

fn main() {
    let mut opt = Opt::from_args();

    std::io::copy(&mut opt.input, &mut opt.output).unwrap();
}
