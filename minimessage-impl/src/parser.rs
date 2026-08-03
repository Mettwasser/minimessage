use std::borrow::Cow;
use std::iter::Peekable;

use crate::error::{Error, Result};
use crate::token::Token;
use crate::tokenizer::Tokenizer;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Expression<'a> {
    Unnamed,
    Named(&'a str),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Node<'a> {
    Element {
        tag: &'a str,
        children: Vec<Node<'a>>,
    },
    Text(Cow<'a, str>),
    Expression(Expression<'a>),
}

pub struct Parser<'a> {
    tokenizer: Peekable<Tokenizer<'a>>,
}

impl<'a> Iterator for Parser<'a> {
    type Item = Result<Node<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        let token = self.peek()?;
        match token {
            Token::Text(_) | Token::Backslash => Some(self.parse_text()),
            Token::AngleOpen => Some(self.parse_element()),
            Token::CurlyOpen => Some(self.parse_expression()),
            _ => None,
        }
    }
}

impl<'a> Parser<'a> {
    pub fn new(tokenizer: Tokenizer<'a>) -> Self {
        Self {
            tokenizer: tokenizer.peekable(),
        }
    }

    fn peek(&mut self) -> Option<&Token<'a>> {
        match self.tokenizer.peek()? {
            Ok(t) => Some(t),
            Err(_) => None,
        }
    }

    fn advance(&mut self) -> Option<Token<'a>> {
        match self.tokenizer.next()? {
            Ok(t) => Some(t),
            Err(_) => None,
        }
    }

    fn parse_text(&mut self) -> Result<Node<'a>> {
        let mut parts = Vec::new();

        loop {
            match self.peek() {
                // double-match avoids shared ref issues with peek + advance
                Some(Token::Text(_)) => {
                    if let Token::Text(s) = self.advance().unwrap() {
                        parts.push(s);
                    }
                }
                Some(Token::Backslash) => {
                    self.advance();
                    match self.advance() {
                        Some(Token::CurlyOpen) => parts.push("{"),
                        Some(Token::CurlyClose) => parts.push("}"),
                        Some(Token::Backslash) => parts.push("\\"),
                        Some(Token::AngleOpen) => parts.push("<"),
                        Some(Token::AngleClose) => parts.push(">"),
                        Some(Token::Slash) => parts.push("/"),
                        Some(Token::Text(s)) => parts.push(s),
                        None => return Err(Error::UnexpectedEof),
                    }
                }
                _ => break,
            }
        }

        let text = if parts.len() == 1 {
            Cow::Borrowed(parts[0])
        } else {
            Cow::Owned(parts.concat())
        };

        Ok(Node::Text(text))
    }

    fn parse_element(&mut self) -> Result<Node<'a>> {
        self.advance();
        let tag = match self.advance() {
            Some(Token::Text(t)) => t,
            Some(t) => return Err(Error::InvalidToken(t.to_owned())),
            None => return Err(Error::UnexpectedEof),
        };
        match self.advance() {
            Some(Token::AngleClose) => {}
            Some(t) => return Err(Error::InvalidToken(t.to_owned())),
            None => return Err(Error::UnexpectedEof),
        }
        let children = self.parse_children_until(tag)?;
        Ok(Node::Element { tag, children })
    }

    fn parse_children_until(&mut self, closing_tag: &'a str) -> Result<Vec<Node<'a>>> {
        let mut children = Vec::new();
        loop {
            if let Some(Token::AngleOpen) = self.peek() {
                self.advance();

                if let Some(Token::Slash) = self.peek() {
                    self.advance();

                    let tag = match self.advance() {
                        Some(Token::Text(t)) => t,
                        Some(t) => return Err(Error::InvalidToken(t.to_owned())),
                        None => return Err(Error::UnexpectedEof),
                    };
                    match self.advance() {
                        Some(Token::AngleClose) => {}
                        Some(t) => return Err(Error::InvalidToken(t.to_owned())),
                        None => return Err(Error::UnexpectedEof),
                    }
                    if tag != closing_tag {
                        return Err(Error::MismatchedTag {
                            expected: closing_tag.to_string(),
                            found: tag.to_string(),
                        });
                    }

                    return Ok(children);
                }
                children.push(self.parse_element_body()?);
            } else {
                match self.next() {
                    Some(child) => children.push(child?),
                    None => return Err(Error::UnexpectedEof),
                }
            }
        }
    }

    fn parse_element_body(&mut self) -> Result<Node<'a>> {
        let tag = match self.advance() {
            Some(Token::Text(t)) => t,
            Some(t) => return Err(Error::InvalidToken(t.to_owned())),
            None => return Err(Error::UnexpectedEof),
        };
        match self.advance() {
            Some(Token::AngleClose) => {}
            Some(t) => return Err(Error::InvalidToken(t.to_owned())),
            None => return Err(Error::UnexpectedEof),
        }
        let children = self.parse_children_until(tag)?;
        Ok(Node::Element { tag, children })
    }

    fn parse_expression(&mut self) -> Result<Node<'a>> {
        self.advance();
        match self.peek() {
            Some(Token::CurlyClose) => {
                self.advance();
                Ok(Node::Expression(Expression::Unnamed))
            }
            Some(_) => {
                let name = match self.advance() {
                    Some(Token::Text(t)) => t,
                    Some(t) => return Err(Error::InvalidToken(t.to_owned())),
                    None => return Err(Error::UnexpectedEof),
                };
                match self.advance() {
                    Some(Token::CurlyClose) => {}
                    Some(t) => return Err(Error::InvalidToken(t.to_owned())),
                    None => return Err(Error::UnexpectedEof),
                }
                Ok(Node::Expression(Expression::Named(name)))
            }
            None => Err(Error::UnexpectedEof),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_nodes(input: &str) -> Vec<Node<'_>> {
        Parser::new(Tokenizer::new(input))
            .collect::<Result<Vec<_>>>()
            .unwrap()
    }

    fn nodes(input: &str) -> Vec<Node<'_>> {
        get_nodes(input)
    }

    #[test]
    fn short_text() {
        assert_eq!(
            nodes("Hello world"),
            vec![Node::Text(Cow::Borrowed("Hello world"))]
        );
    }

    #[test]
    fn simple_element() {
        assert_eq!(
            nodes("<p>Hello</p>"),
            vec![Node::Element {
                tag: "p",
                children: vec![Node::Text(Cow::Borrowed("Hello"))],
            }],
        );
    }

    #[test]
    fn nested_elements() {
        assert_eq!(
            nodes("<div><p>Hello <b>bold</b> world</p><span>hi</span></div>"),
            vec![Node::Element {
                tag: "div",
                children: vec![
                    Node::Element {
                        tag: "p",
                        children: vec![
                            Node::Text(Cow::Borrowed("Hello ")),
                            Node::Element {
                                tag: "b",
                                children: vec![Node::Text(Cow::Borrowed("bold"))],
                            },
                            Node::Text(Cow::Borrowed(" world")),
                        ],
                    },
                    Node::Element {
                        tag: "span",
                        children: vec![Node::Text(Cow::Borrowed("hi"))],
                    },
                ],
            }],
        );
    }

    #[test]
    fn expression_only() {
        assert_eq!(
            nodes("{name}"),
            vec![Node::Expression(Expression::Named("name"))],
        );
    }

    #[test]
    fn escaped_char() {
        assert_eq!(
            nodes(r"escaped \{ brace"),
            vec![Node::Text(Cow::Owned("escaped { brace".to_string()))],
        );
    }

    #[test]
    fn complex_message() {
        let nodes = get_nodes(
            r"Hello <blue>{name}, welcome to <orange>Rust</orange>!</blue> with an escaped \{",
        );

        let expected = vec![
            Node::Text(Cow::Borrowed("Hello ")),
            Node::Element {
                tag: "blue",
                children: vec![
                    Node::Expression(Expression::Named("name")),
                    Node::Text(Cow::Borrowed(", welcome to ")),
                    Node::Element {
                        tag: "orange",
                        children: vec![Node::Text(Cow::Borrowed("Rust"))],
                    },
                    Node::Text(Cow::Borrowed("!")),
                ],
            },
            Node::Text(Cow::Owned(" with an escaped {".to_string())),
        ];

        assert_eq!(nodes, expected);
    }

    #[test]
    fn long_plain_text() {
        let input = "hello world ".repeat(100);
        let expected = input.clone();
        let nodes = get_nodes(&input);
        assert_eq!(nodes, vec![Node::Text(Cow::Owned(expected))]);
    }
}
