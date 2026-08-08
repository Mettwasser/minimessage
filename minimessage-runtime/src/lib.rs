use core::fmt;
use std::collections::HashMap;

use minimessage_impl::{
    parser::{Expression, Node, Parser},
    style::{self, ClickEvent, Decoration, HoverEvent, NamedColor, Special, SpecialError},
    tokenizer::Tokenizer,
};
use pumpkin_plugin_api::{common::NamedColor as PumpkingNamedColor, text::TextComponent};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub enum Error {
    #[error("A positional arg for position {_0} could not be found")]
    PositionalArgNotFound(usize),

    #[error("A positional arg for named argument {_0} could not be found")]
    NamedArgNotFound(String),

    Special(#[from] SpecialError),

    Parser(#[from] minimessage_impl::error::Error),
}

fn map_named_color(color: NamedColor) -> PumpkingNamedColor {
    match color {
        NamedColor::Black => PumpkingNamedColor::Black,
        NamedColor::DarkBlue => PumpkingNamedColor::DarkBlue,
        NamedColor::DarkGreen => PumpkingNamedColor::DarkGreen,
        NamedColor::DarkAqua => PumpkingNamedColor::DarkAqua,
        NamedColor::DarkRed => PumpkingNamedColor::DarkRed,
        NamedColor::DarkPurple => PumpkingNamedColor::DarkPurple,
        NamedColor::Gold => PumpkingNamedColor::Gold,
        NamedColor::Gray => PumpkingNamedColor::Gray,
        NamedColor::DarkGray => PumpkingNamedColor::DarkGray,
        NamedColor::Blue => PumpkingNamedColor::Blue,
        NamedColor::Green => PumpkingNamedColor::Green,
        NamedColor::Aqua => PumpkingNamedColor::Aqua,
        NamedColor::Red => PumpkingNamedColor::Red,
        NamedColor::LightPurple => PumpkingNamedColor::LightPurple,
        NamedColor::Yellow => PumpkingNamedColor::Yellow,
        NamedColor::White => PumpkingNamedColor::White,
    }
}

#[derive(Default)]
pub struct ArgumentCollection<'a> {
    positionals: Vec<Box<dyn fmt::Display + 'a>>,
    named: HashMap<&'a str, Box<dyn fmt::Display + 'a>>,
}

impl<'a> ArgumentCollection<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn positional(mut self, data: impl fmt::Display + 'a) -> Self {
        self.positionals.push(Box::new(data));
        self
    }

    pub fn named(mut self, name: &'a str, data: impl fmt::Display + 'a) -> Self {
        self.named.insert(name, Box::new(data));
        self
    }

    fn resolve(&self, name: &str) -> Option<String> {
        self.named.get(name).map(|val| val.to_string())
    }

    fn resolve_unnamed(&self, pos: usize) -> Option<String> {
        self.positionals.get(pos).map(|val| val.to_string())
    }
}

pub struct FormatArgs<'a> {
    args: ArgumentCollection<'a>,
    pos: usize,
}

impl<'a> FormatArgs<'a> {
    pub fn new() -> Self {
        FormatArgs {
            args: ArgumentCollection::new(),
            pos: 0,
        }
    }

    pub fn new_args(args: ArgumentCollection<'a>) -> Self {
        FormatArgs { args, pos: 0 }
    }

    fn resolve(&mut self, name: &str) -> Option<String> {
        self.args.resolve(name)
    }

    fn resolve_unnamed(&mut self) -> Option<String> {
        let value = self.args.resolve_unnamed(self.pos)?;
        self.pos += 1;
        Some(value.to_string())
    }
}

fn build_node(node: &Node, args: &mut FormatArgs) -> Result<TextComponent> {
    match node {
        Node::Text(text) => Ok(TextComponent::text(text.as_ref())),

        Node::Expression(Expression::Named(name)) => {
            let text = args
                .resolve(name)
                .ok_or_else(|| Error::NamedArgNotFound(name.to_string()))?;

            Ok(TextComponent::text(&text))
        },

        Node::Expression(Expression::Unnamed) => {
            let text = args
                .resolve_unnamed()
                .ok_or(Error::PositionalArgNotFound(args.pos))?;

            Ok(TextComponent::text(&text))
        },

        Node::Element {
            tag,
            children,
            tag_descriptors,
        } => {
            let elem = TextComponent::text("");

            if let Some(decoration) = style::tag_to_decoration(tag) {
                match decoration {
                    Decoration::Bold => elem.bold(true),
                    Decoration::Italic => elem.italic(true),
                    Decoration::Underlined => elem.underlined(true),
                    Decoration::Strikethrough => elem.strikethrough(true),
                    Decoration::Obfuscated => elem.obfuscated(true),
                };
            } else if !tag_descriptors.is_empty() {
                let special = Special::from_descriptor(tag, tag_descriptors.clone())?;
                apply_special(&elem, special, args)?;
            } else if let Some(color) = style::tag_to_named_color(tag) {
                elem.color_named(map_named_color(color));
            }

            for child in children {
                let child_comp = build_node(child, args)?;
                elem.add_child(child_comp);
            }

            Ok(elem)
        },
    }
}

fn apply_special(
    component: &TextComponent,
    special: Special,
    _args: &mut FormatArgs,
) -> Result<()> {
    match special {
        Special::Click(click) => match click {
            ClickEvent::OpenUrl(url) => {
                component.click_open_url(&url);
            },
            ClickEvent::RunCommand(command) => {
                component.click_run_command(&command);
            },
            ClickEvent::SuggestCommand(command) => {
                component.click_suggest_command(&command);
            },
            ClickEvent::CopyToClipboard(text) => {
                component.click_copy_to_clipboard(&text);
            },
            ClickEvent::__Empty => {},
        },
        Special::Hover(hover) => match hover {
            HoverEvent::ShowText(text) => {
                let text_comp = parse_hover_text(&text)?;
                component.hover_show_text(text_comp);
            },
            HoverEvent::ShowItem(item) => {
                component.hover_show_item(&item);
            },
            HoverEvent::ShowEntity {
                entity_type,
                id,
                name,
            } => {
                if let Some(n) = name {
                    let name_comp = parse_hover_text(&n)?;
                    component.hover_show_entity(&entity_type, &id, Some(name_comp));
                } else {
                    component.hover_show_entity(&entity_type, &id, None::<TextComponent>);
                }
            },
            HoverEvent::__Empty => {},
        },
    }
    Ok(())
}

fn parse_hover_text(text: &str) -> Result<TextComponent> {
    let nodes = Parser::new(Tokenizer::new(text)).collect::<std::result::Result<Vec<_>, _>>()?;
    let root = TextComponent::text("");
    let mut args = FormatArgs::new();
    for node in &nodes {
        let child = build_node(node, &mut args)?;
        root.add_child(child);
    }
    Ok(root)
}

fn build_component(nodes: &[Node], args: &mut FormatArgs) -> Result<TextComponent> {
    let root = TextComponent::text("");
    for node in nodes {
        let child = build_node(node, args)?;
        root.add_child(child);
    }
    Ok(root)
}

pub fn deserialize(input: &str) -> Result<TextComponent> {
    deserialize_with_args(input, ArgumentCollection::new())
}

pub fn deserialize_with_args(input: &str, args: ArgumentCollection<'_>) -> Result<TextComponent> {
    let nodes =
        Parser::new(Tokenizer::new(input)).collect::<minimessage_impl::error::Result<Vec<_>>>()?;
    let mut fmt_args = FormatArgs::new_args(args);
    build_component(&nodes, &mut fmt_args)
}
