use std::{borrow::Cow, collections::HashMap, num::ParseIntError, str::FromStr};

use serde::Serialize;
use strum::{EnumDiscriminants, EnumString};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NamedColor {
    Black,
    DarkBlue,
    DarkGreen,
    DarkAqua,
    DarkRed,
    DarkPurple,
    Gold,
    Gray,
    DarkGray,
    Blue,
    Green,
    Aqua,
    Red,
    LightPurple,
    Yellow,
    White,
}

pub fn tag_to_named_color(tag: &str) -> Option<NamedColor> {
    Some(match tag {
        "black" => NamedColor::Black,
        "dark_blue" => NamedColor::DarkBlue,
        "dark_green" => NamedColor::DarkGreen,
        "dark_aqua" => NamedColor::DarkAqua,
        "dark_red" => NamedColor::DarkRed,
        "dark_purple" => NamedColor::DarkPurple,
        "gold" => NamedColor::Gold,
        "gray" => NamedColor::Gray,
        "dark_gray" => NamedColor::DarkGray,
        "blue" => NamedColor::Blue,
        "green" => NamedColor::Green,
        "aqua" => NamedColor::Aqua,
        "red" => NamedColor::Red,
        "light_purple" => NamedColor::LightPurple,
        "yellow" => NamedColor::Yellow,
        "white" => NamedColor::White,
        _ => return None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Decoration {
    Bold,
    Italic,
    Underlined,
    Strikethrough,
    Obfuscated,
}

pub fn tag_to_decoration(tag: &str) -> Option<Decoration> {
    Some(match tag {
        "bold" => Decoration::Bold,
        "italic" => Decoration::Italic,
        "underlined" => Decoration::Underlined,
        "strikethrough" => Decoration::Strikethrough,
        "obfuscated" => Decoration::Obfuscated,
        _ => return None,
    })
}

#[derive(Default, Clone, Debug, EnumDiscriminants, EnumString)]
#[strum_discriminants(derive(EnumString))]
#[strum_discriminants(strum(serialize_all = "snake_case"))]
pub enum ClickEvent {
    OpenUrl(String),
    RunCommand(String),
    SuggestCommand(String),
    CopyToClipboard(String),
    #[default]
    __Empty,
}

impl ClickEvent {
    pub fn try_from_descriptors(args: Vec<Cow<'_, str>>) -> Result<Self, SpecialError> {
        let mut iter = args.into_iter();
        let event = iter.next().ok_or(SpecialError::MissingSpecialType)?;
        let mut args = iter.collect::<Vec<_>>();

        match ClickEventDiscriminants::from_str(&event)? {
            ClickEventDiscriminants::OpenUrl => Ok(Self::OpenUrl(
                args.pop()
                    .ok_or(SpecialError::MissingArgument("url"))?
                    .into(),
            )),
            ClickEventDiscriminants::RunCommand => Ok(Self::RunCommand(
                args.pop()
                    .ok_or(SpecialError::MissingArgument("command"))?
                    .into(),
            )),
            ClickEventDiscriminants::SuggestCommand => Ok(Self::SuggestCommand(
                args.pop()
                    .ok_or(SpecialError::MissingArgument("command"))?
                    .into(),
            )),
            ClickEventDiscriminants::CopyToClipboard => Ok(Self::CopyToClipboard(
                args.pop()
                    .ok_or(SpecialError::MissingArgument("contents"))?
                    .into(),
            )),
            ClickEventDiscriminants::__Empty => Err(SpecialError::InvalidEvent),
        }
    }
}

#[derive(Default, Clone, Debug, EnumDiscriminants, EnumString)]
#[strum_discriminants(derive(EnumString))]
#[strum_discriminants(strum(serialize_all = "snake_case"))]
pub enum HoverEvent {
    ShowEntity {
        entity_type: String,
        id: String,
        name: Option<String>,
    },
    ShowItem(String),
    ShowText(String),
    #[default]
    __Empty,
}

impl HoverEvent {
    pub fn try_from_descriptors(descriptors: Vec<Cow<'_, str>>) -> Result<Self, SpecialError> {
        let mut iter = descriptors.into_iter();
        let event = iter.next().ok_or(SpecialError::MissingSpecialType)?;
        let args = iter.collect::<Vec<_>>();

        match HoverEventDiscriminants::from_str(&event)? {
            HoverEventDiscriminants::ShowText => Ok(Self::ShowText(
                args.into_iter()
                    .next()
                    .ok_or(SpecialError::MissingArgument("text"))?
                    .into(),
            )),
            HoverEventDiscriminants::ShowItem => match args.as_slice() {
                [] => Err(SpecialError::MissingArgument("item")),
                [id] => Ok(Self::ShowItem(id.to_string())),
                [id, count] => {
                    let item = Item::new(format!("minecraft:{id}"), count)?;
                    Ok(Self::ShowItem(fastsnbt::to_string(&item)?))
                },
                [id, count, modifier_tag, modifier_value] => match &**modifier_tag {
                    "enchantments" => {
                        let levels: HashMap<String, i32> = fastsnbt::from_str(modifier_value)?;
                        let mut enchantments = Enchantments::default();
                        for (key, value) in levels {
                            enchantments
                                .levels
                                .insert(format!("minecraft:{key}"), value);
                        }
                        let item = Item::new_with_components(
                            format!("minecraft:{id}"),
                            count,
                            Components { enchantments },
                        )?;
                        Ok(Self::ShowItem(fastsnbt::to_string(&item)?))
                    },
                    modifier => Err(SpecialError::InvalidModifier(modifier.to_owned())),
                },
                args => Err(SpecialError::InvalidArguments(
                    args.iter().map(|cow| cow.to_string()).collect(),
                )),
            },
            HoverEventDiscriminants::ShowEntity => {
                let mut args = args.into_iter();
                Ok(Self::ShowEntity {
                    entity_type: args
                        .next()
                        .ok_or(SpecialError::MissingArgument("entity_type"))?
                        .into(),
                    id: args
                        .next()
                        .ok_or(SpecialError::MissingArgument("id"))?
                        .into(),
                    name: args.next().map(String::from),
                })
            },
            HoverEventDiscriminants::__Empty => Err(SpecialError::InvalidEvent),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color(pub u8, pub u8, pub u8);

impl Color {
    pub fn try_from_descriptors(descriptors: Vec<Cow<'_, str>>) -> Result<Self, SpecialError> {
        let mut iter = descriptors.into_iter();
        let color_string = iter
            .next()
            .ok_or(SpecialError::MissingArgument("color_hex"))?;

        Ok(Self::from_str(&color_string)?)
    }
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub enum ParseColorError {
    #[error("Invalid hex string length")]
    InvalidLength,

    InvalidHex(ParseIntError),
}

impl FromStr for Color {
    type Err = ParseColorError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let hex = s.strip_prefix('#').unwrap_or(s);

        if hex.len() != 6 {
            return Err(ParseColorError::InvalidLength);
        }

        let r = u8::from_str_radix(&hex[0..2], 16).map_err(ParseColorError::InvalidHex)?;
        let g = u8::from_str_radix(&hex[2..4], 16).map_err(ParseColorError::InvalidHex)?;
        let b = u8::from_str_radix(&hex[4..6], 16).map_err(ParseColorError::InvalidHex)?;

        Ok(Color(r, g, b))
    }
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub enum SpecialError {
    #[error("missing the special's event type")]
    MissingSpecialType,

    #[error("unknown special `{_0}`")]
    UnknownTag(String),

    ParseError(#[from] strum::ParseError),
    ParseInt(#[from] ParseIntError),
    ParseColor(#[from] ParseColorError),

    Snbt(#[from] fastsnbt::error::Error),

    #[error("Modifier {_0} is not valid")]
    InvalidModifier(String),

    #[error("The argument {_0} is missing")]
    MissingArgument(&'static str),

    #[error("The arguments are invalid: {_0:?}")]
    InvalidArguments(Vec<String>),

    #[error("An invalid event was reached. Should be unreachable.")]
    InvalidEvent,
}

#[derive(Clone, Debug, EnumDiscriminants)]
#[strum_discriminants(derive(EnumString))]
#[strum_discriminants(strum(serialize_all = "snake_case"))]
pub enum Special {
    Click(ClickEvent),
    Hover(HoverEvent),
    Color(Color),
}

impl Special {
    pub fn from_descriptor(
        tag: &str,
        descriptors: Vec<Cow<'_, str>>,
    ) -> Result<Self, SpecialError> {
        match SpecialDiscriminants::from_str(&tag)? {
            SpecialDiscriminants::Click => Ok(Special::Click(ClickEvent::try_from_descriptors(
                descriptors,
            )?)),
            SpecialDiscriminants::Hover => Ok(Special::Hover(HoverEvent::try_from_descriptors(
                descriptors,
            )?)),
            SpecialDiscriminants::Color => {
                Ok(Special::Color(Color::try_from_descriptors(descriptors)?))
            },
        }
    }
}

#[derive(Debug, PartialEq, Serialize)]
struct Item {
    id: String,
    count: i32,
    components: Components,
}

impl Item {
    pub fn new(id: String, count: &Cow<'_, str>) -> Result<Self, ParseIntError> {
        Ok(Item {
            id,
            count: count.parse()?,
            components: Components::default(),
        })
    }

    pub fn new_with_components(
        id: String,
        count: &Cow<'_, str>,
        components: Components,
    ) -> Result<Self, ParseIntError> {
        Ok(Item {
            id,
            count: count.parse()?,
            components,
        })
    }
}

#[derive(Debug, PartialEq, Serialize, Default)]
struct Components {
    #[serde(rename = "minecraft:enchantments")]
    enchantments: Enchantments,
}

#[derive(Debug, PartialEq, Serialize, Default)]
struct Enchantments {
    levels: HashMap<String, i32>,
}
