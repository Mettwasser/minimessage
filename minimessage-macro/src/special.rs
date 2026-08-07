use std::{borrow::Cow, collections::HashMap, num::ParseIntError, str::FromStr};

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use serde::Serialize;
use strum::{EnumDiscriminants, EnumString};
use syn::Ident;

#[derive(Default, Clone, EnumDiscriminants, EnumString)]
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

impl<'a> ClickEvent {
    fn try_from_descriptors(
        ident: Cow<'a, str>,
        mut args: Vec<Cow<'a, str>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let value = args.pop().ok_or("click event requires one argument")?;

        match ClickEventDiscriminants::from_str(&ident)? {
            ClickEventDiscriminants::OpenUrl => Ok(Self::OpenUrl(value.to_string())),
            ClickEventDiscriminants::RunCommand => Ok(Self::RunCommand(value.to_string())),
            ClickEventDiscriminants::SuggestCommand => Ok(Self::SuggestCommand(value.to_string())),
            ClickEventDiscriminants::CopyToClipboard => {
                Ok(Self::CopyToClipboard(value.to_string()))
            },
            ClickEventDiscriminants::__Empty => {
                Err(format!("invalid click event `{ident}`").into())
            },
        }
    }
}

#[derive(Default, Clone, EnumDiscriminants, EnumString)]
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

impl<'a> HoverEvent {
    fn try_from_descriptors(
        ident: Cow<'_, str>,
        args: Vec<Cow<'_, str>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        match HoverEventDiscriminants::from_str(&ident)? {
            HoverEventDiscriminants::ShowText => Ok(Self::ShowText(
                args.into_iter().next().ok_or("missing text")?.into(),
            )),
            HoverEventDiscriminants::ShowItem => match args.as_slice() {
                [] => Err("missing item".into()),
                [id] => Ok(Self::ShowItem(id.to_string())),
                [id, count] => {
                    let item = Item::new(format!("minecraft:{id}"), count)?;

                    Ok(Self::ShowItem(fastsnbt::to_string(&item)?))
                },
                [id, count, modifier_tag, modifier_value] => match &**modifier_tag {
                    "enchantments" => {
                        let levels: HashMap<String, i32> = fastsnbt::from_str(modifier_value)?;
                        let mut enchantments = Enchantments {
                            levels: HashMap::new(),
                        };

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
                    _ => Err("modifier not found".into()),
                },

                _ => Err("Invalid arguments for show item".into()),
            },
            HoverEventDiscriminants::ShowEntity => {
                let mut args = args.into_iter();

                Ok(Self::ShowEntity {
                    entity_type: args.next().ok_or("missing entity type")?.into(),
                    id: args.next().ok_or("missing id")?.into(),
                    name: args.next().map(String::from),
                })
            },
            HoverEventDiscriminants::__Empty => {
                Err(format!("invalid hover event `{ident}`").into())
            },
        }
    }
}

#[derive(Clone, EnumDiscriminants)]
#[strum_discriminants(derive(EnumString))]
pub enum Special {
    Click(ClickEvent),
    Hover(HoverEvent),
}

impl Special {
    pub fn to_fn_call_code(self, var: Ident) -> TokenStream2 {
        match self {
            Self::Click(click) => match click {
                ClickEvent::OpenUrl(url) => quote! { #var.click_open_url(#url); },
                ClickEvent::RunCommand(command) => quote! { #var.click_run_command(#command); },
                ClickEvent::SuggestCommand(command) => {
                    quote! { #var.click_suggest_command(#command); }
                },
                ClickEvent::CopyToClipboard(text) => {
                    quote! { #var.click_copy_to_clipboard(#text); }
                },
                ClickEvent::__Empty => quote! { compile_error!("No"); },
            },
            Self::Hover(hover) => match hover {
                HoverEvent::ShowEntity {
                    entity_type,
                    id,
                    name,
                } => {
                    let name = name
                        .map(|name| quote!(Some(#name)))
                        .unwrap_or_else(|| quote!(None));

                    quote! { #var.hover_show_entity(#entity_type, #id, #name); }
                },
                HoverEvent::ShowItem(item) => quote! { #var.hover_show_item(#item); },
                HoverEvent::ShowText(text) => quote! { {
                    let temp_text = TextComponent::text(#text);
                    #var.hover_show_text(temp_text);
                } },
                HoverEvent::__Empty => quote! { compile_error!("No"); },
            },
        }
    }

    pub fn from_descriptor(
        tag: &str,
        descriptors: Vec<Cow<'_, str>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut iter = descriptors.into_iter();

        let event = iter.next().ok_or("missing special type")?;
        let args = iter.collect::<Vec<_>>();

        match tag {
            "click" => Ok(Special::Click(ClickEvent::try_from_descriptors(
                event, args,
            )?)),
            "hover" => Ok(Special::Hover(HoverEvent::try_from_descriptors(
                event, args,
            )?)),
            _ => Err(format!("unknown special `{tag}`").into()),
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
            components: Default::default(),
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
