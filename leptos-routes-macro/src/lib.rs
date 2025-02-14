mod gen;
mod module_path;
mod path;
mod route_def;
mod route_macro_args;
mod util;

use crate::module_path::ModulePath;
use crate::path::PathSegments;
use crate::route_def::RouteDef;
use crate::route_macro_args::RouteMacroArgs;
use crate::util::to_pascal_case;
use darling::ast::NestedMeta;
use darling::FromMeta;
use proc_macro::TokenStream;
use proc_macro_error2::proc_macro_error;
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::{parse_macro_input, Expr, Item, ItemMod};

#[proc_macro_attribute]
#[proc_macro_error]
pub fn route(_attr: TokenStream, input: TokenStream) -> TokenStream {
    input
}

// Custom wrapper type for parsing expressions from attributes
#[derive(Debug)]
struct ExprWrapper(Expr);

impl ExprWrapper {
    fn from_value(value: &syn::Lit) -> darling::Result<Self> {
        match value {
            syn::Lit::Str(s) => Self::from_string(&s.value()),
            _ => Err(darling::Error::custom("Expected string literal")),
        }
    }

    fn from_string(value: &str) -> darling::Result<Self> {
        syn::parse_str::<Expr>(value)
            .map(ExprWrapper)
            .map_err(|e| darling::Error::custom(format!("Failed to parse expression: {}", e)))
    }
}

impl FromMeta for ExprWrapper {
    fn from_value(value: &syn::Lit) -> darling::Result<Self> {
        ExprWrapper::from_value(value)
    }

    fn from_string(value: &str) -> darling::Result<Self> {
        ExprWrapper::from_string(value)
    }
}

#[derive(Debug, FromMeta)]
struct RoutesMacroArgs {
    #[darling(default)]
    with_views: bool,

    #[darling(default)]
    fallback: Option<ExprWrapper>,
}

/// This is the entry point for route-declarations. Put it on a module. Declare your routes using
/// the `route` attribute on nested modules. You can freely nest your routes.
///
/// ```
/// use leptos_routes::routes;
///
/// #[routes]
/// pub mod routes {
///
///     #[route("/users")]
///     pub mod users {
///
///         #[route("/:id")]
///         pub mod user {
///
///             #[route("/details")]
///             pub mod details {}
///         }
///     }
/// }
/// ```
#[proc_macro_attribute]
#[proc_macro_error]
pub fn routes(args: TokenStream, input: TokenStream) -> TokenStream {
    let attr_args = match NestedMeta::parse_meta_list(args.into()) {
        Ok(v) => v,
        Err(e) => {
            return TokenStream::from(darling::Error::from(e).write_errors());
        }
    };
    let args = match RoutesMacroArgs::from_list(&attr_args) {
        Ok(v) => v,
        Err(e) => {
            return TokenStream::from(e.write_errors());
        }
    };

    let mut root_mod = parse_macro_input!(input as ItemMod);

    // Make sure we have module contents to work with.
    let (_brace, ref mut content) = match root_mod.content {
        Some((brace, ref mut content)) => (brace, content),
        None => {
            return syn::Error::new_spanned(
                root_mod.ident,
                "routes macro requires a module with a body",
            )
            .to_compile_error()
            .into();
        }
    };

    // Add the route import at the start of the module.
    let route_import: Item = syn::parse_quote! {
        use ::leptos_routes::route;
    };
    content.insert(0, route_import);

    let mut route_defs: Vec<RouteDef> = Vec::new();
    for item in content.iter_mut() {
        if let Item::Mod(child_module) = item {
            add_additional_imports_to_modules(child_module);

            collect_route_definitions(
                child_module,
                None,
                None,
                &mut route_defs,
                ModulePath::root(root_mod.ident.clone()),
            );
        }
    }

    gen::gen_impls(&mut root_mod, args, route_defs);

    let (brace, ref mut content) = match root_mod.content {
        Some((brace, ref mut content)) => (brace, content),
        None => unreachable!("Already checked for empty module"),
    };

    // Reconstruct the module with all additions.
    root_mod.content = Some((brace, content.to_vec()));

    Into::into(quote! { #root_mod })
}

fn add_additional_imports_to_modules(module: &mut ItemMod) {
    if let Some((_, items)) = &mut module.content {
        let imports: Item = syn::parse_quote! {
            use ::leptos_routes::route;
        };
        items.insert(0, imports);

        for item in items.iter_mut() {
            if let Item::Mod(child_module) = item {
                add_additional_imports_to_modules(child_module);
            }
        }
    }
}

fn collect_route_definitions(
    module: &ItemMod,
    parent_path: Option<&str>,
    parent_struct: Option<&syn::Ident>,
    route_defs: &mut Vec<RouteDef>,
    module_path: ModulePath,
) {
    let module_name = &module.ident;
    let vis = &module.vis;

    // Create current module path
    let mut current_module_path = module_path.clone();
    current_module_path.push(module_name.clone());

    let args = match RouteMacroArgs::parse(&module.attrs) {
        None => {
            // This module was not annotated with `#[route]`. Skip it and all potential submodules.
            return;
        }
        Some(args) => args,
    };

    let mut route_def = RouteDef {
        id: uuid::Uuid::new_v4(),
        module_span: module.span(),
        route_ident_span: args.route_ident_span,
        path: args.route_path_segments.clone(),
        path_segments: PathSegments::parse(&args.route_path_segments),
        layout: args.layout,
        layout_span: args.layout_span,
        fallback: args.fallback,
        fallback_span: args.fallback_span,
        view: args.view,
        view_span: args.view_span,
        name: format_ident!("{}", to_pascal_case(&module_name.to_string())),
        parent_struct: match (parent_path, parent_struct) {
            (Some(parent_path), Some(parent_struct)) => {
                Some((parent_path.to_owned(), parent_struct.clone()))
            }
            (None, None) => None,
            _ => panic!("Invalid state"), // TODO: phrase
        },
        vis: vis.clone(),
        found_in_module_path: current_module_path.clone(),
        children: Vec::new(),
    };

    if let Some((_, items)) = &module.content {
        for item in items.iter() {
            if let Item::Mod(child_module) = item {
                collect_route_definitions(
                    child_module,
                    Some(&args.route_path_segments),
                    Some(&route_def.name.clone()),
                    &mut route_def.children,
                    current_module_path.clone(),
                );
            }
        }
    }
    route_defs.push(route_def);
}
