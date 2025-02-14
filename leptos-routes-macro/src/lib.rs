mod expr_wrapper;
mod gen;
mod module_path;
mod path;
mod route_def;
mod route_macro_args;
mod util;

use crate::expr_wrapper::ExprWrapper;
use crate::module_path::ModulePath;
use crate::route_def::{collect_route_definitions, RouteDef};
use darling::ast::NestedMeta;
use darling::FromMeta;
use proc_macro::TokenStream;
use proc_macro_error2::{abort, proc_macro_error};
use quote::quote;
use syn::{parse_macro_input, Item, ItemMod};

#[proc_macro_attribute]
#[proc_macro_error]
pub fn route(_attr: TokenStream, input: TokenStream) -> TokenStream {
    input
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

    let mut root_mod: ItemMod = parse_macro_input!(input as ItemMod);

    // Make sure we have module contents to work with.
    let (_brace, ref mut content) = match root_mod.content {
        Some((brace, ref mut content)) => (brace, content),
        None => {
            abort!(root_mod.ident, "routes macro requires a module with a body");
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
