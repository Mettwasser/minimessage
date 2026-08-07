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
            Token::Text(_) | Token::Backslash | Token::Colon | Token::Slash
                | Token::DoubleQuote | Token::Quote => {
                Some(self.parse_text().map(Node::Text))
            }
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
        self.parse_text_internal(None)
    }

    fn parse_descriptor_text(&mut self, matching_quote: char) -> Result<Cow<'a, str>> {
        self.parse_text_internal(Some(matching_quote))
    }

    fn parse_text_internal(&mut self, break_quote: Option<char>) -> Result<Cow<'a, str>> {
        let mut parts = Vec::new();

        loop {
            match self.peek() {
                Some(Token::DoubleQuote) if break_quote == Some('"') => break,
                Some(Token::Quote) if break_quote == Some('\'') => break,
                Some(Token::DoubleQuote) => {
                    self.advance();
                    parts.push("\"");
                }
                Some(Token::Quote) => {
                    self.advance();
                    parts.push("'");
                }
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
                        Some(Token::Text(s)) => {
                            if let Some(rest) = s.strip_prefix('n') {
                                parts.push("\n");
                                if !rest.is_empty() {
                                    parts.push(rest);
                                }
                            } else {
                                parts.push(s);
                            }
                        }
                        Some(Token::DoubleQuote) => parts.push("\""),
                        Some(Token::Quote) => parts.push("\'"),
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

    fn is_void_element(tag: &str) -> bool {
        matches!(tag, "newline" | "br")
    }

    fn parse_element_body(&mut self) -> Result<Node<'a>> {
        let tag = expect_token!(self.advance(), Token::Text(t) => t);

        let descriptors = self.parse_descriptors()?;

        expect_token!(self.advance(), Token::AngleClose);

        if Self::is_void_element(tag) {
            return Ok(Node::Text(Cow::Borrowed("\n")));
        }

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

            let descriptor = match self.advance() {
                Some(Token::Text(t)) => Cow::Borrowed(t),
                Some(Token::DoubleQuote) => {
                    let inner_text = self.parse_descriptor_text('"')?;
                    expect_token!(self.advance(), Token::DoubleQuote);
                    inner_text
                }
                Some(Token::Quote) => {
                    let inner_text = self.parse_descriptor_text('\'')?;
                    expect_token!(self.advance(), Token::Quote);
                    inner_text
                }
                Some(tok) => return Err(Error::InvalidToken(tok.into())),
                None => return Err(Error::UnexpectedEof),
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
    fn complex_descriptor_double_quote() {
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
    fn complex_descriptor_quote() {
        assert_eq!(
            nodes(r#"Funny link: <click:open_url:'https://pumpkinmc.org/'></click>"#),
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
    fn descriptor_nested_quote_double_outer() {
        assert_eq!(
            nodes(r#"<p:"hello 'world'">Test</p>"#),
            vec![Node::Element {
                tag: "p",
                tag_descriptors: vec![Cow::Borrowed("hello 'world'")],
                children: vec![Node::Text(Cow::Borrowed("Test"))],
            }],
        );
    }

    #[test]
    fn descriptor_nested_quote_single_outer() {
        assert_eq!(
            nodes(r#"<p:'hello "world"'>Test</p>"#),
            vec![Node::Element {
                tag: "p",
                tag_descriptors: vec![Cow::Borrowed(r#"hello "world""#)],
                children: vec![Node::Text(Cow::Borrowed("Test"))],
            }],
        );
    }

    #[test]
    fn quotes_in_regular_text() {
        assert_eq!(
            nodes("It's a \"nice\" day"),
            vec![Node::Text(Cow::Owned("It's a \"nice\" day".to_owned()))]
        );
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

    #[test]
    fn newline_tag() {
        assert_eq!(nodes("<newline>"), vec![Node::Text(Cow::Borrowed("\n"))]);
    }

    #[test]
    fn br_tag() {
        assert_eq!(nodes("<br>"), vec![Node::Text(Cow::Borrowed("\n"))]);
    }

    #[test]
    fn linebreak_tag() {
        assert_eq!(nodes("<newline>"), vec![Node::Text(Cow::Borrowed("\n"))]);
    }

    #[test]
    fn newline_between_text() {
        assert_eq!(
            nodes("Hello<newline>World"),
            vec![
                Node::Text(Cow::Borrowed("Hello")),
                Node::Text(Cow::Borrowed("\n")),
                Node::Text(Cow::Borrowed("World")),
            ]
        );
    }

    #[test]
    fn escaped_newline() {
        assert_eq!(
            nodes(r"Hello\nWorld"),
            vec![Node::Text(Cow::Owned("Hello\nWorld".to_owned()))]
        );
    }

    #[test]
    fn escaped_newline_at_start() {
        assert_eq!(
            nodes(r"\nHello"),
            vec![Node::Text(Cow::Owned("\nHello".to_owned()))]
        );
    }

    #[test]
    fn escaped_newline_with_trailing() {
        assert_eq!(
            nodes(r"Hello\nWorld\nFoo"),
            vec![Node::Text(Cow::Owned("Hello\nWorld\nFoo".to_owned()))]
        );
    }

    #[test]
    fn multiple_newlines() {
        assert_eq!(
            nodes("A<br>B<br>C<newline>D"),
            vec![
                Node::Text(Cow::Borrowed("A")),
                Node::Text(Cow::Borrowed("\n")),
                Node::Text(Cow::Borrowed("B")),
                Node::Text(Cow::Borrowed("\n")),
                Node::Text(Cow::Borrowed("C")),
                Node::Text(Cow::Borrowed("\n")),
                Node::Text(Cow::Borrowed("D")),
            ]
        );
    }

    #[test]
    fn newline_in_element() {
        assert_eq!(
            nodes("<red>Hello<br>World</red>"),
            vec![Node::Element {
                tag: "red",
                tag_descriptors: vec![],
                children: vec![
                    Node::Text(Cow::Borrowed("Hello")),
                    Node::Text(Cow::Borrowed("\n")),
                    Node::Text(Cow::Borrowed("World")),
                ],
            }]
        );
    }
}
