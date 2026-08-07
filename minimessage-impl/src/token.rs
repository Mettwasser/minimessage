use strum::{EnumDiscriminants, VariantArray};

#[derive(Debug, Clone, PartialEq, EnumDiscriminants)]
#[strum_discriminants(derive(VariantArray))]
pub enum Token<'a> {
    AngleOpen,
    AngleClose,
    CurlyOpen,
    CurlyClose,
    Backslash,
    Slash,
    Colon,
    DoubleQuote,
    Quote,
    Text(&'a str),
}

impl<'a> Token<'_> {
    pub const ILLEGAL_TEXT_CHARS: [char; TokenDiscriminants::VARIANTS.len() - 1] =
        TokenDiscriminants::collect_chars(TokenDiscriminants::VARIANTS);
}

impl<'a> From<Token<'a>> for String {
    fn from(value: Token<'a>) -> Self {
        if let Token::Text(text) = value {
            text.to_owned()
        } else {
            // SAFETY: None Variant can't be reached because of the check above
            TokenDiscriminants::from(value)
                .to_char()
                .unwrap()
                .to_string()
        }
    }
}

impl TokenDiscriminants {
    pub const fn to_char(&self) -> Option<char> {
        let char = match self {
            Self::AngleOpen => '<',
            Self::AngleClose => '>',
            Self::CurlyOpen => '{',
            Self::CurlyClose => '}',
            Self::Backslash => '\\',
            Self::Slash => '/',
            Self::Colon => ':',
            Self::DoubleQuote => '"',
            Self::Quote => '\'',
            Self::Text => return None,
        };

        Some(char)
    }

    const fn collect_chars<const N: usize>(items: &[TokenDiscriminants]) -> [char; N] {
        let mut out = ['\0'; N];
        let mut i = 0;
        let mut j = 0;

        while i < items.len() {
            if let Some(c) = items[i].to_char() {
                out[j] = c;
                j += 1;
            }
            i += 1;
        }

        out
    }
}
