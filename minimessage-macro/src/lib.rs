use std::{env, fs};

use heck::ToPascalCase;
use minimessage_impl::{
    parser::{Expression, Node, Parser},
    style::{self, ClickEvent, Color, Decoration, HoverEvent, Special},
    tokenizer::Tokenizer,
};
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{
    Expr,
    Ident,
    LitStr,
    Token,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

mod keywords {
    syn::custom_keyword!(file);
}

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
    is_file: bool,
    format_str: LitStr,
    args: Vec<FormatArg>,
}

impl Parse for MacroInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let is_file = if input.peek(keywords::file) {
            input.parse::<keywords::file>()?;
            input.parse::<Token![:]>()?;
            true
        } else {
            false
        };

        let format_str = input.parse()?;
        let mut args = Vec::new();

        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            args = Punctuated::<FormatArg, Token![,]>::parse_terminated(input)
                .unwrap_or_default()
                .into_iter()
                .collect();
        }

        Ok(MacroInput {
            format_str,
            args,
            is_file,
        })
    }
}

fn tag_to_color_code(tag: &str) -> TokenStream2 {
    let pascal_case_tag = tag.to_pascal_case();
    let ident = format_ident!("{pascal_case_tag}");
    quote! { NamedColor::#ident }
}

fn decoration_to_fn_call_code(decoration: Decoration, var: &Ident) -> TokenStream2 {
    let fn_name = match decoration {
        Decoration::Bold => quote! { bold(true) },
        Decoration::Italic => quote! { italic(true) },
        Decoration::Underlined => quote! { underlined(true) },
        Decoration::Strikethrough => quote! { strikethrough(true) },
        Decoration::Obfuscated => quote! { obfuscated(true) },
    };
    quote! { #var.#fn_name; }
}

fn special_to_fn_call_code(special: Special, var: Ident) -> TokenStream2 {
    match special {
        Special::Click(click) => match click {
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
        Special::Hover(hover) => match hover {
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
            HoverEvent::ShowText(text) => {
                let nodes = Parser::new(Tokenizer::new(&text))
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .unwrap();
                let root = format_ident!("__hover_text");
                let mut var_counter = 0;
                let mut positional_idx = 0;
                let child_code =
                    generate_nodes(&nodes, &[], &mut positional_idx, &mut var_counter, &root);

                quote! { {
                    let #root = TextComponent::text("");
                    #child_code
                    #var.hover_show_text(#root);
                } }
            },
            HoverEvent::__Empty => quote! { compile_error!("No"); },
        },
        Special::Color(Color(r, g, b)) => {
            quote! {
                #var.color_rgb(RgbColor { r: #r, g: #g, b: #b });
            }
        },
    }
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
            },
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
            },
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
            },
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

                let code_to_insert = if let Some(decoration) = style::tag_to_decoration(tag) {
                    decoration_to_fn_call_code(decoration, &var)
                } else if !tag_descriptors.is_empty() {
                    match Special::from_descriptor(tag, tag_descriptors.clone()) {
                        Ok(special) => special_to_fn_call_code(special, var.clone()),
                        Err(e) => {
                            let msg = e.to_string();
                            quote! { compile_error!(#msg); }
                        },
                    }
                } else {
                    let color = tag_to_color_code(tag);
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
            },
        }
    }
    code
}

#[proc_macro]
pub fn minimessage(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as MacroInput);
    let mut value = input.format_str.value().to_string();

    if input.is_file {
        let path = std::path::Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join(value);
        value = fs::read_to_string(&path)
            .expect(&format!("File not found: {}", path.to_str().unwrap()));
    }

    let nodes = Parser::new(Tokenizer::new(&value))
        .collect::<std::result::Result<Vec<_>, _>>()
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
            use ::pumpkin_plugin_api::{common::NamedColor, text::{TextComponent, RgbColor}};

            let #root = TextComponent::text("");
            #child_code
            #root
        }
    }
    .into()
}
