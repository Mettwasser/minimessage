use std::borrow::Cow;
use std::iter::Peekable;

use crate::error::{Error, Result};
use crate::token::Token;
use crate::tokenizer::Tokenizer;

macro_rules! expect_token {
    ($parser:expr, $($pattern:pat => $result:expr),*) => {
        match $parser {
            $( Some($pattern) => $result, )*
            Some(tok) => return Err(Error::InvalidToken(tok.into())),
            None => return Err(Error::UnexpectedEof),
        }
    };
    ($parser:expr, $($pattern:pat),*) => {
        match $parser {
            Some($( $pattern )|*) => {}
            Some(tok) => return Err(Error::InvalidToken(tok.into())),
            None => return Err(Error::UnexpectedEof),
        }
    };
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expression<'a> {
    Unnamed,
    Named(Cow<'a, str>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Node<'a> {
    Element {
        tag: &'a str,
        tag_descriptors: Vec<Cow<'a, str>>,
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
            Token::Text(_) | Token::Backslash => Some(self.parse_text().map(Node::Text)),
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

    fn parse_text(&mut self) -> Result<Cow<'a, str>> {
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
                        Some(Token::Backslash) => parts.push(r"\"),
                        Some(Token::AngleOpen) => parts.push("<"),
                        Some(Token::AngleClose) => parts.push(">"),
                        Some(Token::Slash) => parts.push(r"/"),
                        Some(Token::Colon) => parts.push(r":"),
                        Some(Token::Text(s)) => parts.push(s),
                        Some(Token::DoubleQuote | Token::Quote) => parts.push("\""),
                        None => return Err(Error::UnexpectedEof),
                    }
                }
                Some(Token::Colon) => {
                    self.advance();
                    parts.push(":");
                }
                Some(Token::Slash) => {
                    self.advance();
                    parts.push("/");
                }
                Some(Token::CurlyOpen) => {
                    self.advance();
                    parts.push("{");
                }
                Some(Token::CurlyClose) => {
                    self.advance();
                    parts.push("}");
                }
                _ => break,
            }
        }

        let text = if parts.len() == 1 {
            Cow::Borrowed(parts[0])
        } else {
            Cow::Owned(parts.concat())
        };

        Ok(text)
    }

    fn parse_element(&mut self) -> Result<Node<'a>> {
        self.advance();
        self.parse_element_body()
    }

    fn parse_element_body(&mut self) -> Result<Node<'a>> {
        let tag = expect_token!(self.advance(), Token::Text(t) => t);

        let descriptors = self.parse_descriptors()?;

        expect_token!(self.advance(), Token::AngleClose);

        let children = self.parse_children_until(tag)?;

        Ok(Node::Element {
            tag,
            children,
            tag_descriptors: descriptors,
        })
    }

    fn parse_descriptors(&mut self) -> Result<Vec<Cow<'a, str>>> {
        let mut descriptors = Vec::new();

        while self.peek() == Some(&Token::Colon) {
            self.advance(); // consume `:`

            let descriptor = expect_token! {
                self.advance(),
                Token::Text(t) => Cow::Borrowed(t),
                Token::DoubleQuote | Token::Quote=> {
                    let inner_text = self.parse_text()?;
                    expect_token!(self.advance(), Token::DoubleQuote | Token::Quote);
                    inner_text
                }
            };

            descriptors.push(descriptor);
        }

        Ok(descriptors)
    }

    fn parse_children_until(&mut self, closing_tag: &'a str) -> Result<Vec<Node<'a>>> {
        let mut children = Vec::new();
        loop {
            if let Some(Token::AngleOpen) = self.peek() {
                self.advance();

                if let Some(Token::Slash) = self.peek() {
                    self.advance();

                    let tag = expect_token!(self.advance(), Token::Text(t) => t);

                    expect_token!(self.advance(), Token::AngleClose);

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
                    None => return Ok(children),
                }
            }
        }
    }

    fn parse_expression(&mut self) -> Result<Node<'a>> {
        self.advance();
        match self.peek() {
            Some(Token::CurlyClose) => {
                self.advance();
                Ok(Node::Expression(Expression::Unnamed))
            }
            Some(_) => {
                let name = expect_token!(self.advance(), Token::Text(t) => t);

                let name = if self.peek() == Some(&Token::Colon) {
                    self.advance();
                    let format_spec = expect_token!(self.advance(), Token::Text(t) => t);
                    Cow::Owned(format!("{}:{}", name, format_spec))
                } else {
                    Cow::Borrowed(name)
                };

                expect_token!(self.advance(), Token::CurlyClose);
                Ok(Node::Expression(Expression::Named(name)))
            }
            None => Err(Error::UnexpectedEof),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nodes_fallible(input: &str) -> Result<Vec<Node<'_>>> {
        Parser::new(Tokenizer::new(input)).collect::<Result<Vec<_>>>()
    }

    fn nodes(input: &str) -> Vec<Node<'_>> {
        nodes_fallible(input).unwrap()
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
                tag_descriptors: vec![],
                children: vec![Node::Text(Cow::Borrowed("Hello"))],
            }],
        );
    }

    #[test]
    fn element_simplified_style() {
        assert_eq!(
            nodes("<blue>Hello <yellow>I'm yellow and <red>red"),
            vec![Node::Element {
                tag: "blue",
                tag_descriptors: vec![],
                children: vec![
                    Node::Text(Cow::Borrowed("Hello "),),
                    Node::Element {
                        tag: "yellow",
                        tag_descriptors: vec![],
                        children: vec![
                            Node::Text(Cow::Borrowed("I'm yellow and ")),
                            Node::Element {
                                tag: "red",
                                tag_descriptors: Vec::new(),
                                children: vec![Node::Text(Cow::Borrowed("red"))]
                            }
                        ]
                    }
                ],
            }],
        );
    }

    #[test]
    fn nested_elements() {
        assert_eq!(
            nodes("<div><p>Hello <b>bold</b> world</p><span>hi</span></div>"),
            vec![Node::Element {
                tag: "div",
                tag_descriptors: vec![],
                children: vec![
                    Node::Element {
                        tag: "p",
                        tag_descriptors: vec![],
                        children: vec![
                            Node::Text(Cow::Borrowed("Hello ")),
                            Node::Element {
                                tag: "b",
                                tag_descriptors: vec![],
                                children: vec![Node::Text(Cow::Borrowed("bold"))],
                            },
                            Node::Text(Cow::Borrowed(" world")),
                        ],
                    },
                    Node::Element {
                        tag: "span",
                        tag_descriptors: vec![],
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
            vec![Node::Expression(Expression::Named(Cow::Borrowed("name")))],
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
        let nodes = nodes(
            r"Hello <blue>{name}, welcome to <orange>Rust</orange>!</blue> with an escaped \{",
        );

        let expected = vec![
            Node::Text(Cow::Borrowed("Hello ")),
            Node::Element {
                tag: "blue",
                tag_descriptors: vec![],
                children: vec![
                    Node::Expression(Expression::Named(Cow::Borrowed("name"))),
                    Node::Text(Cow::Borrowed(", welcome to ")),
                    Node::Element {
                        tag: "orange",
                        tag_descriptors: vec![],
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
        let nodes = nodes(&input);
        assert_eq!(nodes, vec![Node::Text(Cow::Owned(expected))]);
    }

    #[test]
    fn simple_element_with_one_descriptor() {
        assert_eq!(
            nodes("<p:testingiscool>Hello</p>"),
            vec![Node::Element {
                tag: "p",
                tag_descriptors: vec![Cow::Borrowed("testingiscool")],
                children: vec![Node::Text(Cow::Borrowed("Hello"))],
            }],
        );
    }

    #[test]
    fn simple_element_with_one_descriptor_int() {
        assert_eq!(
            nodes("<p:1>Hello</p>"),
            vec![Node::Element {
                tag: "p",
                tag_descriptors: vec![Cow::Borrowed("1")],
                children: vec![Node::Text(Cow::Borrowed("Hello"))],
            }],
        );
    }

    #[test]
    fn simple_element_with_multiple_descriptors() {
        assert_eq!(
            nodes(
                r#"<p:testingiscool:"now with a string":an_ident:"and another string">Hello</p>"#
            ),
            vec![Node::Element {
                tag: "p",
                tag_descriptors: vec![
                    Cow::Borrowed("testingiscool"),
                    Cow::Borrowed("now with a string".into()),
                    Cow::Borrowed("an_ident"),
                    Cow::Borrowed("and another string".into())
                ],
                children: vec![Node::Text(Cow::Borrowed("Hello"))],
            }],
        );
    }

    #[test]
    fn simple_element_with_descriptor_colon() {
        assert_eq!(
            nodes(r#"<p:"string with : a colon">Hello</p>"#),
            vec![Node::Element {
                tag: "p",
                tag_descriptors: vec![Cow::Owned(r#"string with : a colon"#.to_owned())],
                children: vec![Node::Text(Cow::Borrowed("Hello"))],
            }],
        );
    }

    #[test]
    fn simple_element_with_descriptor_escaped_colon() {
        assert_eq!(
            nodes(r#"<p:"string with \: a colon">Hello</p>"#),
            vec![Node::Element {
                tag: "p",
                tag_descriptors: vec![Cow::Owned(r#"string with : a colon"#.to_owned())],
                children: vec![Node::Text(Cow::Borrowed("Hello"))],
            }],
        );
    }

    #[test]
    fn complex_descriptor() {
        assert_eq!(
            nodes(r#"Funny link: <click:open_url:"https://pumpkinmc.org/"></click>"#),
            vec![
                Node::Text("Funny link: ".into()),
                Node::Element {
                    tag: "click",
                    tag_descriptors: vec![
                        Cow::Borrowed("open_url"),
                        Cow::Borrowed("https://pumpkinmc.org/")
                    ],
                    children: vec![]
                }
            ]
        )
    }

    #[test]
    fn test_complex_message() {
        assert_eq!(
            nodes(
                "<blue>Hello there, <red><bold>{}</bold></red>!</blue> <yellow>Here's your shiny bold number: <bold>{number:.2}</bold></yellow>"
            ),
            vec![
                Node::Element {
                    tag: "blue",
                    tag_descriptors: vec![],
                    children: vec![
                        Node::Text(Cow::Borrowed("Hello there, ")),
                        Node::Element {
                            tag: "red",
                            tag_descriptors: vec![],
                            children: vec![Node::Element {
                                tag: "bold",
                                tag_descriptors: vec![],
                                children: vec![Node::Expression(Expression::Unnamed)]
                            }]
                        },
                        Node::Text("!".into())
                    ],
                },
                Node::Text(" ".into()),
                Node::Element {
                    tag: "yellow",
                    tag_descriptors: vec![],
                    children: vec![
                        Node::Text("Here's your shiny bold number: ".into()),
                        Node::Element {
                            tag: "bold",
                            tag_descriptors: vec![],
                            children: vec![Node::Expression(Expression::Named(Cow::Owned(
                                "number:.2".to_owned()
                            )))]
                        }
                    ]
                }
            ]
        );
    }
}
