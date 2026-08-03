#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Token<'a> {
    AngleOpen,
    AngleClose,
    CurlyOpen,
    CurlyClose,
    Text(&'a str),
    Backslash,
    Slash,
}

impl<'a> Token<'_> {
    pub fn to_owned(&self) -> TokenOwned {
        match self {
            Self::AngleOpen => TokenOwned::AngleOpen,
            Self::AngleClose => TokenOwned::AngleClose,
            Self::CurlyOpen => TokenOwned::CurlyOpen,
            Self::CurlyClose => TokenOwned::CurlyClose,
            Self::Text(s) => TokenOwned::Text(s.to_string()),
            Self::Backslash => TokenOwned::Backslash,
            Self::Slash => TokenOwned::Slash,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenOwned {
    AngleOpen,
    AngleClose,
    CurlyOpen,
    CurlyClose,
    Text(String),
    Backslash,
    Slash,
}
