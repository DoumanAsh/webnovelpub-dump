use arg::Args;

#[derive(Args, Debug)]
///Utility to download text of the webnovelpub novels
pub struct Cli {
    #[arg(required)]
    ///Id of the novel to dump (e.g. the-novels-extra-07082217)
    pub novel: String,
    #[arg(long, default_value = "2")]
    ///Specifies delay in seconds between attempt to download again. Defaults to 2 seconds
    pub retry_delay: u64,
}

impl Cli {
    #[inline]
    pub fn new<'a, T: IntoIterator<Item = &'a str>>(args: T) -> Result<Self, bool> {
        let args = args.into_iter();

        Cli::from_args(args).map_err(|err| match err.is_help() {
            true => {
                println!("{}", Cli::HELP);
                false
            },
            false => {
                eprintln!("{}", err);
                true
            },
        })
    }
}
