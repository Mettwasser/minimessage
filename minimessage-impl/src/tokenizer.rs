use std::{iter::Peekable, str::CharIndices};

use crate::{error::Result, token::Token};

const ILLEGAL_TEXT_CHARS: &[char] = &['<', '>', '{', '}', '\\', '/'];

pub struct Tokenizer<'a> {
    pub(crate) input: &'a str,
    char_indices: Peekable<CharIndices<'a>>,
}

impl<'a> Tokenizer<'a> {
    #[must_use]
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            char_indices: input.char_indices().peekable(),
        }
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = Result<Token<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        let (token_start_idx, char) = self.char_indices.next()?;

        let token_result = match char {
            '<' => Ok(Token::AngleOpen),
            '>' => Ok(Token::AngleClose),
            '{' => Ok(Token::CurlyOpen),
            '}' => Ok(Token::CurlyClose),
            '\\' => Ok(Token::Backslash),
            '/' => Ok(Token::Slash),

            _ => Ok(Token::Text(self.read_string(token_start_idx, char)?)),
        };

        Some(token_result)
    }
}

impl<'a> Tokenizer<'a> {
    fn read_string(&mut self, start_idx: usize, first_char: char) -> Option<&'a str> {
        let mut end_idx = start_idx + first_char.len_utf8();

        while let Some(&(idx, c)) = self.char_indices.peek() {
            if ILLEGAL_TEXT_CHARS.contains(&c) {
                break;
            }

            end_idx = idx + c.len_utf8();
            self.char_indices.next();
        }

        Some(&self.input[start_idx..end_idx])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokenize(input: &str) -> Vec<Token<'_>> {
        Tokenizer::new(input).collect::<Result<Vec<_>>>().unwrap()
    }

    #[test]
    fn empty_string() {
        assert!(tokenize("").is_empty());
    }

    #[test]
    fn single_char_text() {
        assert_eq!(tokenize("a"), vec![Token::Text("a")]);
    }

    #[test]
    fn text_between_angle_brackets() {
        assert_eq!(
            tokenize("<a>"),
            vec![Token::AngleOpen, Token::Text("a"), Token::AngleClose],
        );
    }

    #[test]
    fn text_between_angle_brackets_multi_char() {
        assert_eq!(
            tokenize("<abcde>"),
            vec![Token::AngleOpen, Token::Text("abcde"), Token::AngleClose],
        );
    }

    #[test]
    fn each_special_char() {
        assert_eq!(tokenize("<"), vec![Token::AngleOpen]);
        assert_eq!(tokenize(">"), vec![Token::AngleClose]);
        assert_eq!(tokenize("{"), vec![Token::CurlyOpen]);
        assert_eq!(tokenize("}"), vec![Token::CurlyClose]);
        assert_eq!(tokenize("\\"), vec![Token::Backslash]);
        assert_eq!(tokenize("/"), vec![Token::Slash]);
    }

    #[test]
    fn consecutive_special_chars() {
        assert_eq!(
            tokenize("<>{}"),
            vec![
                Token::AngleOpen,
                Token::AngleClose,
                Token::CurlyOpen,
                Token::CurlyClose,
            ],
        );
    }

    #[test]
    fn text_at_start_no_special() {
        assert_eq!(tokenize("hello"), vec![Token::Text("hello")],);
    }

    #[test]
    fn text_followed_by_special() {
        assert_eq!(
            tokenize("text<"),
            vec![Token::Text("text"), Token::AngleOpen],
        );
    }

    #[test]
    fn text_preceded_by_special() {
        assert_eq!(
            tokenize(">text"),
            vec![Token::AngleClose, Token::Text("text")],
        );
    }

    #[test]
    fn text_between_consecutive_special_chars() {
        assert_eq!(
            tokenize("<a>b{c}d/e"),
            vec![
                Token::AngleOpen,
                Token::Text("a"),
                Token::AngleClose,
                Token::Text("b"),
                Token::CurlyOpen,
                Token::Text("c"),
                Token::CurlyClose,
                Token::Text("d"),
                Token::Slash,
                Token::Text("e"),
            ],
        );
    }

    #[test]
    fn text_with_spaces_and_punctuation() {
        assert_eq!(tokenize("hello world!"), vec![Token::Text("hello world!")],);
    }

    #[test]
    fn text_with_numbers() {
        assert_eq!(tokenize("abc123"), vec![Token::Text("abc123")]);
    }

    #[test]
    fn special_chars_sandwich_text() {
        assert_eq!(
            tokenize("<text>"),
            vec![Token::AngleOpen, Token::Text("text"), Token::AngleClose],
        );
    }

    #[test]
    fn multiple_text_segments() {
        assert_eq!(
            tokenize("a>b"),
            vec![Token::Text("a"), Token::AngleClose, Token::Text("b")],
        );
    }

    #[test]
    fn unicode_text() {
        assert_eq!(tokenize("héllo"), vec![Token::Text("héllo")]);
    }

    #[test]
    fn unicode_text_between_special() {
        assert_eq!(
            tokenize("<héllo>"),
            vec![Token::AngleOpen, Token::Text("héllo"), Token::AngleClose,],
        );
    }

    #[test]
    fn unicode_multibyte_surrounding_special() {
        assert_eq!(
            tokenize("ñ<ü"),
            vec![Token::Text("ñ"), Token::AngleOpen, Token::Text("ü")],
        );
    }

    #[test]
    fn special_then_text_then_special_no_overlap() {
        assert_eq!(
            tokenize("{x}"),
            vec![Token::CurlyOpen, Token::Text("x"), Token::CurlyClose],
        );
    }

    #[test]
    fn backslash_and_slash_mixed() {
        assert_eq!(
            tokenize("a\\b/c"),
            vec![
                Token::Text("a"),
                Token::Backslash,
                Token::Text("b"),
                Token::Slash,
                Token::Text("c"),
            ],
        );
    }

    #[test]
    fn tokenize_is_consuming() {
        let mut tokenizer = Tokenizer::new("<a>");
        assert_eq!(tokenizer.next().unwrap().unwrap(), Token::AngleOpen);
        assert_eq!(tokenizer.next().unwrap().unwrap(), Token::Text("a"));
        assert_eq!(tokenizer.next().unwrap().unwrap(), Token::AngleClose);
        assert!(tokenizer.next().is_none());
    }

    #[test]
    fn all_special_no_text() {
        assert_eq!(
            tokenize("<>{}"),
            vec![
                Token::AngleOpen,
                Token::AngleClose,
                Token::CurlyOpen,
                Token::CurlyClose,
            ],
        );
    }

    #[test]
    fn new_does_not_panic() {
        let _ = Tokenizer::new("");
        let _ = Tokenizer::new("<>{}");
    }
}
