use crate::gen::all_routes_enum::generate_route_enum;
use crate::gen::route_struct::generate_route_struct;
use crate::gen::router::generate_routes_component;
use crate::route_def::{flatten, RouteDef};
use crate::RoutesMacroArgs;
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

        let src_mod = find_src_module(root_mod, route_def.found_in_module_path.without_first())
            .expect("present");

        insert_into_module(src_mod, struct_def);
        insert_into_module(src_mod, struct_impl);
    }

    // Generate a "Route" enum listing all possible routes.
    let all_routes_enum = generate_route_enum(&route_defs);
    insert_into_module(root_mod, all_routes_enum);

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
    insert_into_module(root_mod, routes_fn);
}

pub fn find_src_module<'a>(
    module: &'a mut ItemMod,
    path: &[syn::Ident],
) -> Option<&'a mut ItemMod> {
    if path.is_empty() {
        return Some(module);
    }

    if let Some((_, items)) = &mut module.content {
        for item in items.iter_mut() {
            if let Item::Mod(child_module) = item {
                if child_module.ident == path[0] {
                    return find_src_module(child_module, &path[1..]);
                }
            }
        }
    }

    None
}

pub fn insert_into_module(module: &mut ItemMod, ts: proc_macro2::TokenStream) {
    match syn::parse2::<Item>(ts) {
        Ok(item) => {
            if let Some((_, items)) = &mut module.content {
                items.push(item);
            } else {
                abort_call_site!("Expected module to have content");
            }
        }
        Err(e) => abort_call_site!(e),
    }
}
