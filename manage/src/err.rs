//! Error management with [`error_chain`].

error_chain::error_chain! {
    types {
        Error, ErrorKind, ResExt, Res;
    }
    foreign_links {
        Io(std::io::Error);
        HttpRequest(reqwest::Error);
    }
}
impl From<Error> for Vec<Error> {
    fn from(e: Error) -> Self {
        vec![e]
    }
}
