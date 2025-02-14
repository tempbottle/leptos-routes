use crate::route_def::{flatten, RouteDef};
use crate::util::to_pascal_case;
use quote::{format_ident, quote};

pub fn generate_route_enum(route_defs: &[RouteDef]) -> proc_macro2::TokenStream {
    let mut all_routes_variants = Vec::new();
    for route_def in flatten(route_defs) {
        let struct_name = &route_def.name;

        let paths = &route_def.found_in_module_path.without_first();

        let mut variant_name = paths
            .iter()
            .next()
            .cloned()
            .map(|it| format_ident!("{}", to_pascal_case(&it.to_string())));
        if variant_name.is_some() {
            for next in paths.iter().skip(1) {
                variant_name = Some(format_ident!(
                    "{}{}",
                    variant_name.unwrap(),
                    to_pascal_case(&next.to_string())
                ));
            }
        }
        let variant_name = variant_name
            .map(|it| format_ident!("{it}{struct_name}"))
            .unwrap_or(struct_name.clone());
        let path = quote! { #(#paths::)*#struct_name };

        all_routes_variants.push(quote! {
            #variant_name(#path),
        })
    }
    let all_routes_enum = quote! {
        pub enum Route {
            #(#all_routes_variants)*
        }
    };
    all_routes_enum
}
