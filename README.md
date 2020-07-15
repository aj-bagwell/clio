# clio
clio is a rust library for parsing CLI file names.

It implements the standard unix conventions of when the file name is `-` then
sending the data to stdin/stdout


    // a cat replacement
    fn main() -> clio::Result<()> {
        let args: Vec<_> = std::env::args_os().collect();
        let mut input = clio::Input::new(&args[1])?;
        std::io::copy(&mut input, &mut std::io::stdout())?;
        Ok(())
    }
