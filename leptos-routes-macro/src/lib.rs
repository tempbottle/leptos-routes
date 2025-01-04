use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Attribute, Item, ItemMod, Visibility};

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
pub fn routes(_attr: TokenStream, input: TokenStream) -> TokenStream {
    let mut root_mod = parse_macro_input!(input as ItemMod);

    // Make sure we have module contents to work with
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

    // Add the route import at the start of the module
    let route_import: Item = syn::parse_quote! {
        use ::leptos_routes::route;
    };
    content.insert(0, route_import);

    let mut route_infos = Vec::new();

    // Process all submodules first
    for item in content.iter_mut() {
        if let Item::Mod(sub_mod) = item {
            if let Some(route_path) = extract_route_attr(&sub_mod.attrs) {
                collect_routes(
                    sub_mod,
                    &route_path,
                    None,
                    &mut route_infos,
                    vec![root_mod.ident.clone()],
                );
            }
        }
    }

    // Drop the ref so that we can borrow root_mod again.
    //drop(content);

    fn insert_into_module(module: &mut ItemMod, path: &[syn::Ident], item: Item) {
        if path.is_empty() {
            if let Some((_, items)) = &mut module.content {
                items.push(item);
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
    }

    // Generate the route structs
    for info in &route_infos {
        let struct_name = &info.name;
        let path = &info.path;
        let vis = &info.vis;

        let segments = parse_path_segments(&info.path);
        let path_type = generate_path_type(&segments);
        let segment_count = segments.len();

        let struct_def = quote! {
            #[derive(Clone, Copy, Debug)]
            #vis struct #struct_name;
        };

        // For the format string, we need to handle both:
        // 1. The original path segments from self.path() for static segments
        // 2. The function parameters for dynamic segments
        fn create(
            segments: &[PathSegment],
            format_str: &mut String,
            format_args: &mut Vec<proc_macro2::TokenStream>,
        ) {
            for (i, seg) in segments.iter().enumerate() {
                let segment_var = format_ident!("segment_{}", i);
                match seg {
                    PathSegment::Static(_) => {
                        format_str.push_str("/{}");
                        format_args
                            .push(quote! { ::leptos_router::AsPath::as_path(&(#segment_var).0) });
                    }
                    PathSegment::Param(name) => {
                        format_str.push_str("/{}");
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
                        format_str.push_str("/{}");
                        let name = format_ident!("{}", sanitize_identifier(name));
                        format_args.push(quote! { #name });
                    }
                }
            }
        }

        let struct_impl = match &info.parent_struct {
            Some(parent) => {
                let all_params = collect_params_through_hierarchy(&route_infos, info);

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
                        !segments.iter().any(|seg| {
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
                create(&segments, &mut format_str, &mut format_args);

                let segment_vars = (0..segments.len()).map(|i| format_ident!("segment_{}", i));

                quote! {
                    impl #struct_name {
                        pub fn path(&self) -> #path_type {
                            ::leptos_router::path!(#path)
                        }

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
                let segment_vars = (0..segment_count).map(|i| format_ident!("segment_{}", i));

                // Collect parameters for dynamic segments
                let params: Vec<_> = segments
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
                create(&segments, &mut format_str, &mut format_args);

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

        match (
            syn::parse2::<Item>(struct_def),
            syn::parse2::<Item>(struct_impl),
        ) {
            (Ok(struct_item), Ok(impl_item)) => {
                // Skip the 'routes' module in the path when inserting
                insert_into_module(
                    &mut root_mod,
                    &info.found_in_module_path[1..info.found_in_module_path.len() - 1],
                    struct_item,
                );
                insert_into_module(
                    &mut root_mod,
                    &info.found_in_module_path[1..info.found_in_module_path.len() - 1],
                    impl_item,
                );
            }
            (Err(e), _) | (_, Err(e)) => return TokenStream::from(e.to_compile_error()),
        }
    }

    let (brace, ref mut content) = match root_mod.content {
        Some((brace, ref mut content)) => (brace, content),
        None => unreachable!("Already checked for empty module"),
    };

    // Reconstruct the module with all additions
    root_mod.content = Some((brace, content.to_vec()));

    Into::into(quote! { #root_mod })
}

#[proc_macro_attribute]
pub fn route(_attr: TokenStream, input: TokenStream) -> TokenStream {
    input
}

#[derive(Debug)]
struct RouteInfo {
    path: String,
    name: syn::Ident,
    parent_struct: Option<syn::Ident>,
    vis: Visibility,
    found_in_module_path: Vec<syn::Ident>,
}

fn collect_routes(
    module: &mut ItemMod,
    current_path: &str,
    parent_struct: Option<&syn::Ident>,
    route_infos: &mut Vec<RouteInfo>,
    module_path: Vec<syn::Ident>,
) {
    let module_name = &module.ident;
    let vis = &module.vis;

    // Create current module path
    let mut current_module_path = module_path.clone();
    current_module_path.push(module_name.clone());

    // Add current module's route
    route_infos.push(RouteInfo {
        path: current_path.to_string(),
        name: format_ident!("{}", to_pascal_case(&module_name.to_string())),
        parent_struct: parent_struct.cloned(),
        vis: vis.clone(),
        found_in_module_path: current_module_path.clone(),
    });

    // Process nested modules
    if let Some((_, items)) = &mut module.content {
        let current_struct = format_ident!("{}", to_pascal_case(&module_name.to_string()));

        // Add route import to the start of this module.
        let route_import: Item = syn::parse_quote! {
            use ::leptos_routes::route;
        };
        items.insert(0, route_import);

        // Process child modules
        for item in items.iter_mut() {
            if let Item::Mod(sub_mod) = item {
                if let Some(route_path) = extract_route_attr(&sub_mod.attrs) {
                    collect_routes(
                        sub_mod,
                        &route_path,
                        Some(&current_struct),
                        route_infos,
                        current_module_path.clone(),
                    );
                }
            }
        }
    }
}

fn extract_route_attr(attrs: &[Attribute]) -> Option<String> {
    attrs
        .iter()
        .find(|attr| attr.path().is_ident("route"))
        .and_then(|attr| attr.parse_args::<syn::LitStr>().ok().map(|lit| lit.value()))
}

fn to_pascal_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true;

    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.extend(c.to_uppercase());
            capitalize_next = false;
        } else {
            result.extend(c.to_lowercase());
        }
    }

    result
}

fn sanitize_identifier(name: &str) -> String {
    const RUST_KEYWORDS: &[&str] = &[
        "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn",
        "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
        "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe",
        "use", "where", "while",
    ];

    if RUST_KEYWORDS.contains(&name) {
        format!("{}_", name)
    } else {
        name.to_string()
    }
}

#[derive(Debug, Clone)]
struct ParamInfo {
    name: String,
    is_optional: bool,
    #[expect(unused)]
    is_wildcard: bool,
}

/// Collect parameters from a route and its parents.
fn collect_params_through_hierarchy(
    route_infos: &[RouteInfo],
    current_info: &RouteInfo,
) -> Vec<ParamInfo> {
    let mut params = Vec::new();
    let mut current = Some(current_info);

    while let Some(info) = current {
        let segments = parse_path_segments(&info.path);
        for seg in segments {
            match seg {
                PathSegment::Param(name) => params.push(ParamInfo {
                    name,
                    is_optional: false,
                    is_wildcard: false,
                }),
                PathSegment::OptionalParam(name) => params.push(ParamInfo {
                    name,
                    is_optional: true,
                    is_wildcard: false,
                }),
                PathSegment::Wildcard(name) => params.push(ParamInfo {
                    name,
                    is_optional: false,
                    is_wildcard: true,
                }),
                PathSegment::Static(_) => {}
            }
        }

        current = info
            .parent_struct
            .as_ref()
            .and_then(|parent_name| route_infos.iter().find(|info| info.name == *parent_name));
    }

    // Reverse to get parent params first
    params.reverse();
    params
}

#[derive(Debug)]
enum PathSegment {
    Static(#[expect(unused)] String),
    Param(String),
    OptionalParam(String),
    Wildcard(String),
}

fn parse_path_segments(path: &str) -> Vec<PathSegment> {
    path.split('/')
        .filter(|s| !s.is_empty())
        .map(|segment| {
            if let Some(param) = segment.strip_prefix(':') {
                if let Some(optional) = param.strip_suffix('?') {
                    PathSegment::OptionalParam(optional.to_string())
                } else {
                    PathSegment::Param(param.to_string())
                }
            } else if let Some(wildcard) = segment.strip_prefix('*') {
                PathSegment::Wildcard(wildcard.to_string())
            } else {
                PathSegment::Static(segment.to_string())
            }
        })
        .collect()
}

fn generate_path_type(segments: &[PathSegment]) -> proc_macro2::TokenStream {
    let segment_types = segments.iter().map(|segment| match segment {
        PathSegment::Static(_) => quote!(::leptos_router::StaticSegment<&'static str>),
        PathSegment::Param(_) => quote!(::leptos_router::ParamSegment),
        PathSegment::OptionalParam(_) => quote!(::leptos_router::OptionalParamSegment),
        PathSegment::Wildcard(_) => quote!(::leptos_router::WildcardSegment),
    });

    match segments.len() {
        0 => quote!(()),
        _ => quote!((#(#segment_types,)*)),
    }
}
