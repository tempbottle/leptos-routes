use crate::path::{ParamInfo, PathSegment, PathSegments};
use crate::route_def::RouteDef;
use crate::util::sanitize_identifier;
use quote::{format_ident, quote};

// For the format string, we need to handle both:
// 1. The original path segments from self.path() for static segments
// 2. The function parameters for dynamic segments
fn create(
    segments: &PathSegments,
    format_str: &mut String,
    format_args: &mut Vec<proc_macro2::TokenStream>,
    has_parent_with_empty_path: bool,
) {
    if segments.segments.is_empty() {
        format_str.push_str("/");
        return;
    }
    for (i, seg) in segments.segments.iter().enumerate() {
        let segment_var = format_ident!("segment_{}", i);
        match seg {
            PathSegment::Static(_) => {
                if i == 0 && has_parent_with_empty_path {
                    format_str.push_str("{}");
                } else {
                    format_str.push_str("/{}");
                }
                format_args.push(quote! { ::leptos_router::AsPath::as_path(&(#segment_var).0) });
            }
            PathSegment::Param(name) => {
                if i == 0 && has_parent_with_empty_path {
                    format_str.push_str("{}");
                } else {
                    format_str.push_str("/{}");
                }
                let name = format_ident!("{}", sanitize_identifier(name));
                format_args.push(quote! { #name });
            }
            PathSegment::OptionalParam(name) => {
                format_str.push_str("{}");
                let name = format_ident!("{}", sanitize_identifier(name));
                format_args.push(quote! {
                    if let Some(val) = #name {
                        format!("/{}", val)
                    } else {
                        String::new()
                    }
                });
            }
            PathSegment::Wildcard(name) => {
                if i == 0 && has_parent_with_empty_path {
                    format_str.push_str("{}");
                } else {
                    format_str.push_str("/{}");
                }
                let name = format_ident!("{}", sanitize_identifier(name));
                format_args.push(quote! { #name });
            }
        }
    }
}

pub fn generate_route_struct(
    route_def: &RouteDef,
    route_defs: &[RouteDef],
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    let struct_name = &route_def.name;
    let path = &route_def.path;
    let vis = &route_def.vis;

    let path_segments = &route_def.path_segments;
    let path_segment_count = path_segments.segments.len();
    let path_type = path_segments.generate_path_type();

    let struct_def = quote! {
        #[doc = #path]
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #vis struct #struct_name;
    };

    let struct_impl = match &route_def.parent_struct {
        Some((parent_path, parent)) => {
            let all_params = ParamInfo::collect_params_through_hierarchy(&route_defs, route_def);

            let params = all_params.iter().map(|p| {
                let name = format_ident!("{}", sanitize_identifier(&p.name));
                if p.is_optional {
                    quote! { #name: Option<&str> }
                } else {
                    quote! { #name: &str }
                }
            });

            let parent_params = all_params
                .iter()
                .take_while(|p| {
                    !path_segments.segments.iter().any(|seg| {
                        matches!(seg,
                            PathSegment::Param(name) |
                            PathSegment::OptionalParam(name) |
                            PathSegment::Wildcard(name) if name == &p.name
                        )
                    })
                })
                .map(|p| format_ident!("{}", sanitize_identifier(&p.name)));

            let mut format_str = String::new();
            format_str.push_str("{}"); // Capturing the parent path!
            let mut format_args = Vec::new();
            create(
                &path_segments,
                &mut format_str,
                &mut format_args,
                parent_path.is_empty() || parent_path == "/",
            );

            let segment_vars = (0..path_segment_count).map(|i| format_ident!("segment_{}", i));

            quote! {
                impl #struct_name {
                    pub fn path(&self) -> #path_type {
                        ::leptos_router::path!(#path)
                    }

                    // TODO add full_path

                    pub fn materialize(&self, #(#params),*) -> String {
                        let parent = super::#parent;
                        let parent_path = parent.materialize(#(#parent_params),*);
                        let (#(#segment_vars,)*) = self.path();
                        format!(#format_str, parent_path, #(#format_args),*)
                    }
                }
            }
        }
        None => {
            // For each segment, we need to track:
            // 1. The segment type (for the path() return type)
            // 2. Whether it needs a parameter in materialize()
            // 3. How to convert it using AsPath
            let segment_vars = (0..path_segment_count).map(|i| format_ident!("segment_{}", i));

            // Collect parameters for dynamic segments
            let params: Vec<_> = path_segments
                .segments
                .iter()
                .filter_map(|seg| match seg {
                    PathSegment::Param(name) => {
                        let name = format_ident!("{}", sanitize_identifier(name));
                        Some(quote! { #name: &str })
                    }
                    PathSegment::OptionalParam(name) => {
                        let name = format_ident!("{}", sanitize_identifier(name));
                        Some(quote! { #name: Option<&str> })
                    }
                    PathSegment::Wildcard(name) => {
                        let name = format_ident!("{}", sanitize_identifier(name));
                        Some(quote! { #name: &str })
                    }
                    PathSegment::Static(_) => None,
                })
                .collect();

            let mut format_str = String::new();
            let mut format_args = Vec::new();
            create(&path_segments, &mut format_str, &mut format_args, false);

            quote! {
                impl #struct_name {
                    pub fn path(&self) -> #path_type {
                        ::leptos_router::path!(#path)
                    }

                    pub fn materialize(&self, #(#params),*) -> String {
                        let (#(#segment_vars,)*) = self.path();
                        format!(#format_str, #(#format_args),*)
                    }
                }
            }
        }
    };

    (struct_def, struct_impl)
}
