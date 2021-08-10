//! Error management with [`error_chain`].

error_chain::error_chain! {
    types {
        Error, ErrorKind, ResExt, Res;
    }
    foreign_links {
        Io(std::io::Error);
    }
}
