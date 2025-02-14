use proc_macro2::Span;
use proc_macro_error2::abort;
use crate::ExprWrapper;
use syn::{Attribute, Expr};

pub struct RouteMacroArgs {
    pub route_ident_span: Span,

    /// A path, defined like: "/" or "/users"
    pub route_path_segments: String,

    /// A wrapper view, defined like: "wrap=MainLayout" or "wrap=|| view! { <MainLayout/> }"
    pub layout: Option<Expr>,
    pub layout_span: Option<Span>,

    pub fallback: Option<Expr>,
    pub fallback_span: Option<Span>,

    /// The route view, defined like: "view=SomePage" or "view=|| view! { <SomePage/> }"
    pub view: Option<Expr>,
    pub view_span: Option<Span>,
}

impl RouteMacroArgs {
    pub fn parse(attrs: &[Attribute]) -> Option<RouteMacroArgs> {
        attrs
            .iter()
            .find(|attr| attr.path().is_ident("route"))
            .and_then(|attr| {
                let ident = attr.path().get_ident().unwrap();

                attr.parse_args_with(|input: syn::parse::ParseStream| {
                    //panic!("Input: {:?}", content);
                    let mut path: Option<String> = None;
                    let mut layout: Option<Expr> = None;
                    let mut layout_span: Option<Span> = None;
                    let mut fallback: Option<Expr> = None;
                    let mut fallback_span: Option<Span> = None;
                    let mut view: Option<Expr> = None;
                    let mut view_span: Option<Span> = None;

                    while !input.is_empty() {
                        let lookahead = input.lookahead1();
                        if lookahead.peek(syn::LitStr) {
                            let lit: syn::LitStr = input.parse()?;
                            path = Some(lit.value());
                        } else if lookahead.peek(syn::Ident) {
                            let ident: syn::Ident = input.parse()?;
                            if ident == "view" {
                                let _ = input.parse::<syn::Token![=]>()?;
                                let lit = input.parse::<syn::Lit>().expect("expect lit");
                                view = Some(ExprWrapper::from_value(&lit)?.0);
                                view_span = Some(ident.span());
                            } else if ident == "layout" {
                                let _ = input.parse::<syn::Token![=]>()?;
                                let lit = input.parse::<syn::Lit>()?;
                                layout = Some(ExprWrapper::from_value(&lit)?.0);
                                layout_span = Some(ident.span());
                            } else if ident == "fallback" {
                                let _ = input.parse::<syn::Token![=]>()?;
                                let lit = input.parse::<syn::Lit>()?;
                                fallback = Some(ExprWrapper::from_value(&lit)?.0);
                                fallback_span = Some(ident.span());
                            } else {
                                abort!(ident.span(), "Unexpected ident: \"{}\". Expected one of \"layout\", \"fallback\" or \"view\".", ident.to_string());
                            }
                        } else {
                            abort!(input.span(), "Unexpected additional macro input. Remove these tokens.");
                        }

                        if !input.is_empty() {
                            let _: syn::Token![,] = input.parse()?;
                        }
                    }
                    let path = path.expect("expect path to be present");

                    Ok(RouteMacroArgs {
                        route_ident_span: ident.span(),
                        route_path_segments: path,
                        layout,
                        layout_span,
                        fallback,
                        fallback_span,
                        view,
                        view_span,
                    })
                })
                .ok()
            })
    }
}
