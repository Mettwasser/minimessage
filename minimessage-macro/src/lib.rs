use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{
    Expr, Ident, LitStr, Token,
    parse::{Parse, ParseStream},
};

use minimessage_impl::{
    parser::{Expression, Node, Parser},
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
            while !input.is_empty() {
                args.push(input.parse()?);
                if input.peek(Token![,]) {
                    input.parse::<Token![,]>()?;
                } else {
                    break;
                }
            }
        }
        Ok(MacroInput { format_str, args })
    }
}

fn tag_to_color(tag: &str) -> TokenStream2 {
    match tag {
        "black" => quote! { NamedColor::Black },
        "dark_blue" => quote! { NamedColor::DarkBlue },
        "dark_green" => quote! { NamedColor::DarkGreen },
        "dark_aqua" => quote! { NamedColor::DarkAqua },
        "dark_red" => quote! { NamedColor::DarkRed },
        "dark_purple" => quote! { NamedColor::DarkPurple },
        "gold" => quote! { NamedColor::Gold },
        "gray" => quote! { NamedColor::Gray },
        "dark_gray" => quote! { NamedColor::DarkGray },
        "blue" => quote! { NamedColor::Blue },
        "green" => quote! { NamedColor::Green },
        "aqua" => quote! { NamedColor::Aqua },
        "red" => quote! { NamedColor::Red },
        "light_purple" => quote! { NamedColor::LightPurple },
        "yellow" => quote! { NamedColor::Yellow },
        "white" => quote! { NamedColor::White },
        _ => panic!("unknown color tag: `{tag}`"),
    }
}

fn resolve_named(
    name: &str,
    args: &[FormatArg],
    positional_idx: &mut usize,
) -> (TokenStream2, LitStr) {
    let (value_part, format_spec) = name.split_once(':').map_or((name, ""), |(v, s)| (v, s));

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
            Node::Element { tag, children } => {
                let color = tag_to_color(tag);
                let var = format_ident!("__c{}", {
                    *var_counter += 1;
                    *var_counter - 1
                });
                let child_code = generate_nodes(children, args, positional_idx, var_counter, &var);
                code.extend(quote! {
                    let #var = TextComponent::text("");
                    #var.color_named(#color);
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
