# clio

clio is a rust library for parsing CLI file names.

It implements the standard unix conventions of when the file name is `"-"` then sending the
data to stdin/stdout as appropriate

# Usage

[`Input`](crate::Input)s and [`Output`](crate::Input)s can be created directly from args in [`args_os`](std::env::args_os).
They will error if the file cannot be opened for any reason

```
// a cat replacement
fn main() -> clio::Result<()> {
    for arg in std::env::args_os() {
        let mut input = clio::Input::new(&arg)?;
        std::io::copy(&mut input, &mut std::io::stdout())?;
    }
    Ok(())
}
```

If you want to defer opening the file you can use [`InputPath`](crate::InputPath)s and [`OutputPath`](crate::OutputPath)s.
This avoid leaving empty Output files around if you error out very early.
These check that the path exists, is a file and could in theory be opened when created to get
nicer error messages from clap. Since that leaves room for
[TOCTTOU](https://en.wikipedia.org/wiki/Time-of-check_to_time-of-use) bugs, they will
still return a [`Err`](std::result::Result::Err) if something has changed when it comes time
to actually open the file.

With the `clap-parse` feature they are also designed to be used with [clap 3.2+](https://docs.rs/clap).

See the [older docs](https://docs.rs/clio/0.2.2/clio/index.html#usage) for examples of older [clap](https://docs.rs/clap)/[structopt](https://docs.rs/structopt)

```
# #[cfg(feature="clap-parse")]{
use clap::Parser;
use clio::*;

#[derive(Parser)]
#[clap(name = "cat")]
struct Opt {
    /// Input file, use '-' for stdin
    #[clap(value_parser, default_value="-")]
    input: Input,

    /// Output file '-' for stdout
    #[clap(long, short, value_parser, default_value="-")]
    output: Output,
}

fn main() {
    let mut opt = Opt::parse();

    std::io::copy(&mut opt.input, &mut opt.output).unwrap();
}
# }
```

# Alternative crates

## Nameless

[Nameless](https://docs.rs/nameless) is an alternative to clap that provides full-service command-line parsing. This means you just write a main function with arguments with the types you want, add a conventional documentation comment, and it uses the magic of procedural macros to take care of the rest.

It's input and output streams have the many of the same features as clio (e.g. '-' for stdin) but also support transparently decompressing inputs, and more remote options such as `scp://`

## Patharg

If you are as horified as I am by the amount of code in this crate for what feels like it should have been a very simple task, then [`patharg`](https://docs.rs/patharg) is a much lighter crate that works with clap for treating '-' as stdin/stdout.

It does not open the file, or otherwise validate the path until you ask it avoiding TOCTTOU issues but in the process looses the nice clap error messages.

It also avoids a whole pile of complexity for dealing with seeking and guessing up front if the input supports seeking.

Also watch out patharg has no custom clap ValueParser so older versions of clap will convert via a String so path will need to be valid utf-8 which is not guarnatied by linux nor windows.

## Either

If all you really need is support mapping `'-'` to `stdin()` try this lovely function distilled from [`patharg`](https://docs.rs/patharg).

It works becuase [either](https://docs.rs/either) has helpfully added `impl`s for many common traits when both sides implement them.

```
    use either::Either;
    use std::io;
    use std::ffi::OsStr;
    use std::fs::File;

    pub fn open(path: &OsStr) -> io::Result<impl io::BufRead> {
        Ok(if path == "-" {
            Either::Left(io::stdin().lock())
        } else {
            Either::Right(io::BufReader::new(File::open(path)?))
        })
    }
```

The corresponding `create` function is left as an exercise for the reader.

# Features

### `clap-parse`

Implements [`ValueParserFactory`](https://docs.rs/clap/latest/clap/builder/trait.ValueParserFactory.html) for all the types and
adds a bad implementation of [`Clone`] to all types as well to keep `clap` happy.

## HTTP Client

If a url is passed to [`Input::new`](crate::Input::new) then it will perform and HTTP `GET`. This has the advantage vs just piping in the output of curl as you know the input size, and can infer related urls, e.g. get the `Cargo.lock` to match the `Cargo.toml`.

If a url is passed to [`Output::new`](crate::Output::new) then it will perform and HTTP `PUT`.
The main advantage over just piping to curl is you can use [`OutputPath::create_with_len`](crate::OutputPath::create_with_len) to set the size before the upload starts e.g.
needed if you are sending a file to S3.

### `http-ureq`

bundles in [ureq](https://docs.rs/ureq) as a HTTP client.

### `http-curl`

bundles in [curl](https://docs.rs/curl) as a HTTP client.
