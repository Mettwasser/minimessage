# minimessage
A (partial) implementation of minimessage for Pumpkin using a macro that checks the syntax at compile time!
The macro is fashioned just like `format!`.
```rs
minimessage!("<blue>Hello {}!</blue>", user.name);
```


## Dynamic Rencering
Currently this isn't supported out of the box.
However, this library additionally provides a tokenizer and parser so you can DIY.

An example of how to use the tokenizer & parser:
```rs
fn get_nodes(input: &str) -> Vec<Node<'_>> {
    Parser::new(Tokenizer::new(input))
        .collect::<Result<Vec<_>>>()
        .unwrap()
}

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
```
