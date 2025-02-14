use crate::gen::all_routes_enum::generate_route_enum;
use crate::gen::route_struct::generate_route_struct;
use crate::gen::router::generate_routes_component;
use crate::route_def::{flatten, RouteDef};
use crate::RoutesMacroArgs;
use itertools::Itertools;
use proc_macro_error2::abort_call_site;
use quote::quote;
use syn::{Item, ItemMod};

pub mod all_routes_enum;
pub mod route_struct;
pub mod router;

pub fn gen_impls(root_mod: &mut ItemMod, args: RoutesMacroArgs, route_defs: Vec<RouteDef>) {
    // Generate the individual route structs.
    for route_def in flatten(&route_defs) {
        let (struct_def, struct_impl) = generate_route_struct(route_def, &route_defs);

        try_insert_into_module(
            root_mod,
            route_def.found_in_module_path.without_first(),
            struct_def,
        );
        try_insert_into_module(
            root_mod,
            route_def.found_in_module_path.without_first(),
            struct_impl,
        );
    }

    // Generate a "Route" enum listing all possible routes.
    let all_routes_enum = generate_route_enum(&route_defs);
    try_insert_into_module(root_mod, &[], all_routes_enum);

    // Generate a "Router" implementation.
    let routes_fn = if args.with_views {
        generate_routes_component(&route_defs, args.fallback) // .map(|f| syn::parse_str(f.suffix()).unwrap())
    } else {
        quote! {
            /// Not implemented!
            ///
            /// Use `#[routes(with_views, fallback="SomeComponent")] ...`
            /// for this function to be generated.
            pub fn generated_routes() -> ! {
                unimplemented!();
            }
        }
    };
    try_insert_into_module(root_mod, &[], routes_fn);
}

pub fn try_insert_into_module(
    module: &mut ItemMod,
    path: &[syn::Ident],
    ts: proc_macro2::TokenStream,
) {
    match syn::parse2::<Item>(ts) {
        Ok(item) => {
            insert_into_module(module, path, item);
        }
        Err(e) => abort_call_site!(e),
    }
}

pub fn insert_into_module(module: &mut ItemMod, path: &[syn::Ident], item: Item) {
    if path.is_empty() {
        if let Some((_, items)) = &mut module.content {
            items.push(item);
        } else {
            abort_call_site!("Expected module to have content");
        }
        return;
    }

    if let Some((_, items)) = &mut module.content {
        for sub_item in items.iter_mut() {
            if let Item::Mod(sub_mod) = sub_item {
                if sub_mod.ident == path[0] {
                    insert_into_module(sub_mod, &path[1..], item);
                    return;
                }
            }
        }
    }

    abort_call_site!(
        "Could not find path '{}' in module {}. Could not insert new item {item:?}.",
        path.iter().map(|it| it.to_string()).join("::"),
        module.ident.to_string()
    );
}
