use std::str::FromStr;

use heck::ToPascalCase;
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use strum::{EnumDiscriminants, EnumString};
use syn::{
    Expr, Ident, LitStr, Token,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

use minimessage_impl::{
    parser::{Descriptor, Expression, Node, Parser},
    tokenizer::Tokenizer,
};

struct FormatArg {
    ident: Option<Ident>,
    expr: Expr,
}

impl Parse for FormatArg {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(Ident) && input.peek2(Token![=]) {
            let ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let expr = input.parse()?;
            Ok(FormatArg {
                ident: Some(ident),
                expr,
            })
        } else {
            let expr = input.parse()?;
            Ok(FormatArg { ident: None, expr })
        }
    }
}

struct MacroInput {
    format_str: LitStr,
    args: Vec<FormatArg>,
}

impl Parse for MacroInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let format_str = input.parse()?;
        let mut args = Vec::new();

        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            args = Punctuated::<FormatArg, Token![,]>::parse_terminated(input)
                .unwrap_or_default()
                .into_iter()
                .collect();
        }

        Ok(MacroInput { format_str, args })
    }
}

fn tag_to_color(tag: &str) -> TokenStream2 {
    let pascal_case_tag = tag.to_pascal_case();
    let ident = format_ident!("{pascal_case_tag}");
    quote! { NamedColor::#ident }
}

#[derive(Default, Clone, EnumDiscriminants, EnumString)]
#[strum_discriminants(derive(EnumString))]
#[strum_discriminants(strum(serialize_all = "snake_case"))]
enum ClickEvent {
    OpenUrl(String),
    RunCommand(String),
    SuggestCommand(String),
    CopyToClipboard(String),

    #[default]
    __Empty,
}

#[derive(Default, Clone, EnumDiscriminants, EnumString)]
#[strum_discriminants(derive(EnumString))]
#[strum_discriminants(strum(serialize_all = "snake_case"))]
enum HoverEvent {
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

#[derive(Clone, EnumDiscriminants)]
#[strum_discriminants(derive(EnumString))]
enum Special {
    Click(ClickEvent),
    Hover(HoverEvent),
}

impl Special {
    fn to_fn_call_code(self, var: Ident) -> TokenStream2 {
        match self {
            Self::Click(click) => match click {
                ClickEvent::OpenUrl(url) => quote! { #var.click_open_url(#url); },
                ClickEvent::RunCommand(command) => quote! { #var.click_run_command(#command); },
                ClickEvent::SuggestCommand(command) => {
                    quote! { #var.click_suggest_command(#command); }
                }
                ClickEvent::CopyToClipboard(text) => {
                    quote! { #var.click_copy_to_clipboard(#text); }
                }
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
                }
                HoverEvent::ShowItem(item) => quote! { #var.hover_show_item(#item); },
                HoverEvent::ShowText(text) => quote! { {
                    let temp_text = TextComponent::text(#text);
                    #var.hover_show_text(temp_text);
                } },
                HoverEvent::__Empty => quote! { compile_error!("No"); },
            },
        }
    }

    fn from_descriptor(
        tag: &str,
        descriptors: Vec<Descriptor<'_>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut iter = descriptors.into_iter();

        let event = match iter.next() {
            Some(Descriptor::Ident(s)) => s,
            Some(_) => return Err("second descriptor must be an identifier".into()),
            None => return Err("missing event type".into()),
        };

        let args = iter
            .map(|d| match d {
                Descriptor::String(s) => s.into_owned(),
                Descriptor::Ident(s) => s.to_owned(),
            })
            .collect::<Vec<_>>();

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

impl ClickEvent {
    fn try_from_descriptors(
        ident: &str,
        mut args: Vec<String>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let value = args.pop().ok_or("click event requires one argument")?;

        match ClickEventDiscriminants::from_str(ident)? {
            ClickEventDiscriminants::OpenUrl => Ok(Self::OpenUrl(value)),
            ClickEventDiscriminants::RunCommand => Ok(Self::RunCommand(value)),
            ClickEventDiscriminants::SuggestCommand => Ok(Self::SuggestCommand(value)),
            ClickEventDiscriminants::CopyToClipboard => Ok(Self::CopyToClipboard(value)),
            ClickEventDiscriminants::__Empty => {
                Err(format!("invalid click event `{ident}`").into())
            }
        }
    }
}

impl HoverEvent {
    fn try_from_descriptors(
        ident: &str,
        args: Vec<String>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        match HoverEventDiscriminants::from_str(ident)? {
            HoverEventDiscriminants::ShowText => Ok(Self::ShowText(
                args.into_iter().next().ok_or("missing text")?,
            )),
            HoverEventDiscriminants::ShowItem => Ok(Self::ShowItem(
                args.into_iter().next().ok_or("missing item")?,
            )),
            HoverEventDiscriminants::ShowEntity => {
                let mut args = args.into_iter();

                Ok(Self::ShowEntity {
                    entity_type: args.next().ok_or("missing entity type")?,
                    id: args.next().ok_or("missing id")?,
                    name: args.next(),
                })
            }
            HoverEventDiscriminants::__Empty => {
                Err(format!("invalid hover event `{ident}`").into())
            }
        }
    }
}

#[derive(Clone)]
enum Decoration {
    Bold,
}

impl Decoration {
    fn to_fn_call_code(self, var: &Ident) -> TokenStream2 {
        let function_to_append = match self {
            Decoration::Bold => quote! { bold(true) },
        };

        quote! { #var.#function_to_append; }
    }
}

fn tag_to_decoration(tag: &str) -> Option<Decoration> {
    Some(match tag {
        "bold" => Decoration::Bold,
        _ => return None,
    })
}

fn resolve_named(
    name: &str,
    args: &[FormatArg],
    positional_idx: &mut usize,
) -> (TokenStream2, LitStr) {
    let (value_part, format_spec) = name.split_once(':').map_or((name, ""), |(v, s)| (v, s));

    if name.contains(':') && format_spec.is_empty() {
        panic!("empty format specifier after ':' in expression {{{name}}}");
    }

    let fmt_lit = if format_spec.is_empty() {
        LitStr::new("{}", Span::call_site())
    } else {
        let fmt = format!("{{:{format_spec}}}");
        LitStr::new(&fmt, Span::call_site())
    };

    let value_expr: TokenStream2 = if value_part.is_empty() {
        let expr = &args
            .get(*positional_idx)
            .unwrap_or_else(|| panic!("missing positional format argument {}", *positional_idx))
            .expr;
        *positional_idx += 1;
        quote! { #expr }
    } else if let Ok(idx) = value_part.parse::<usize>() {
        let expr = &args
            .get(idx)
            .unwrap_or_else(|| panic!("missing positional format argument {idx}"))
            .expr;
        quote! { #expr }
    } else if let Some(arg) = args
        .iter()
        .find(|a| a.ident.as_ref().is_some_and(|i| i == value_part))
    {
        let expr = &arg.expr;
        quote! { #expr }
    } else {
        let ident = Ident::new(value_part, Span::call_site());
        quote! { #ident }
    };

    (value_expr, fmt_lit)
}

fn generate_nodes(
    nodes: &[Node],
    args: &[FormatArg],
    positional_idx: &mut usize,
    var_counter: &mut usize,
    parent: &Ident,
) -> TokenStream2 {
    let mut code = TokenStream2::new();
    for node in nodes {
        match node {
            Node::Text(text) => {
                let var = format_ident!("__c{}", {
                    *var_counter += 1;
                    *var_counter - 1
                });
                code.extend(quote! {
                    let #var = TextComponent::text(#text);
                    #parent.add_child(#var);
                });
            }
            Node::Expression(Expression::Named(name)) => {
                let (value_expr, fmt_lit) = resolve_named(name, args, positional_idx);
                let idx = *var_counter;
                *var_counter += 1;
                let var = format_ident!("__c{idx}");
                let text = format_ident!("__t{idx}");
                code.extend(quote! {
                    let #text = format!(#fmt_lit, #value_expr);
                    let #var = TextComponent::text(&#text);
                    #parent.add_child(#var);
                });
            }
            Node::Expression(Expression::Unnamed) => {
                let expr = args
                    .get(*positional_idx)
                    .unwrap_or_else(|| {
                        panic!("missing positional format argument {}", *positional_idx)
                    })
                    .expr
                    .clone();
                *positional_idx += 1;
                let idx = *var_counter;
                *var_counter += 1;
                let var = format_ident!("__c{idx}");
                let text = format_ident!("__t{idx}");
                code.extend(quote! {
                    let #text = format!("{}", #expr);
                    let #var = TextComponent::text(&#text);
                    #parent.add_child(#var);
                });
            }
            Node::Element {
                tag,
                children,
                tag_descriptors,
            } => {
                let var = format_ident!("__c{}", {
                    *var_counter += 1;
                    *var_counter - 1
                });

                let child_code = generate_nodes(children, args, positional_idx, var_counter, &var);

                let code_to_insert = if let Some(decoration) = tag_to_decoration(tag) {
                    decoration.to_fn_call_code(&var)
                } else if !tag_descriptors.is_empty() {
                    match Special::from_descriptor(tag, tag_descriptors.clone()) {
                        Ok(special) => special.to_fn_call_code(var.clone()),
                        Err(e) => {
                            let msg = e.to_string();
                            quote! { compile_error!(#msg); }
                        }
                    }
                } else {
                    let color = tag_to_color(tag);
                    quote! {
                        #var.color_named(#color);
                    }
                };

                code.extend(quote! {
                    let #var = TextComponent::text("");
                    #code_to_insert
                    #child_code
                    #parent.add_child(#var);
                });
            }
        }
    }
    code
}

#[proc_macro]
pub fn minimessage(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as MacroInput);
    let value = input.format_str.value();

    let nodes = Parser::new(Tokenizer::new(&value))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    let root = format_ident!("__root");
    let mut var_counter = 0;
    let mut positional_idx = 0;
    let child_code = generate_nodes(
        &nodes,
        &input.args,
        &mut positional_idx,
        &mut var_counter,
        &root,
    );

    quote! {
        {
            use ::pumpkin_plugin_api::{common::NamedColor, world::TextComponent};

            let #root = TextComponent::text("");
            #child_code
            #root
        }
    }
    .into()
}
