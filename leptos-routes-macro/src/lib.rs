use darling::ast::NestedMeta;
use darling::FromMeta;
use proc_macro::TokenStream;
use std::iter::from_fn;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Attribute, Expr, Item, ItemMod, PathArguments, Visibility};

// Custom wrapper type for parsing expressions from attributes
#[derive(Debug)]
struct ExprWrapper(Expr);

impl ExprWrapper {
    fn from_value(value: &syn::Lit) -> darling::Result<Self> {
        match value {
            syn::Lit::Str(s) => Self::from_string(&s.value()),
            _ => Err(darling::Error::custom("Expected string literal"))
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

    let mut nested_route_infos: Vec<NestedRouteInfo> = Vec::new();

    // Process all submodules first.
    for item in content.iter_mut() {
        if let Item::Mod(sub_mod) = item {
            if let Some(args) = extract_route_attr(&sub_mod.attrs) {
                collect_routes(
                    sub_mod,
                    args,
                    None,
                    None,
                    &mut nested_route_infos,
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

    // Generate the route structs.
    for info in flatten(&nested_route_infos) {
        let struct_name = &info.name;
        let path = &info.path;
        let vis = &info.vis;

        let segments = parse_path_segments(&info.path);
        let path_type = generate_path_type(&segments);
        let segment_count = segments.len();

        let struct_def = quote! {
            #[doc = #path]
            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            #vis struct #struct_name;
        };

        // For the format string, we need to handle both:
        // 1. The original path segments from self.path() for static segments
        // 2. The function parameters for dynamic segments
        fn create(
            segments: &[PathSegment],
            format_str: &mut String,
            format_args: &mut Vec<proc_macro2::TokenStream>,
            has_parent_with_empty_path: bool,
        ) {
            if segments.is_empty()  {
                format_str.push_str("/");
                return;
            }
            for (i, seg) in segments.iter().enumerate() {
                let segment_var = format_ident!("segment_{}", i);
                match seg {
                    PathSegment::Static(_) => {
                        if i == 0 && has_parent_with_empty_path {
                            format_str.push_str("{}");
                        } else {
                            format_str.push_str("/{}");
                        }
                        format_args
                            .push(quote! { ::leptos_router::AsPath::as_path(&(#segment_var).0) });
                    }
                    PathSegment::Param(name) => {if i == 0 && has_parent_with_empty_path {
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

        let struct_impl = match &info.parent_struct {
            Some((parent_path, parent)) => {
                let all_params = collect_params_through_hierarchy(&nested_route_infos, info);

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
                create(&segments, &mut format_str, &mut format_args, parent_path.is_empty() || parent_path == "/");

                let segment_vars = (0..segments.len()).map(|i| format_ident!("segment_{}", i));

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
                create(&segments, &mut format_str, &mut format_args, false);

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

    let mut all_routes_variants = Vec::new();
    for info in flatten(&nested_route_infos) {
        let struct_name = &info.name;

        let paths = &info.found_in_module_path[1..info.found_in_module_path.len() - 1];

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
    match syn::parse2::<Item>(all_routes_enum) {
        Ok(all_routes_enum) => {
            insert_into_module(&mut root_mod, &[], all_routes_enum);
        }
        Err(e) => return TokenStream::from(e.to_compile_error()),
    }

    // Generate a "Router" implementation
    let routes_fn = if args.with_views {
        generate_routes_component(&nested_route_infos, args.fallback) // .map(|f| syn::parse_str(f.suffix()).unwrap())
    } else {
        quote! {
            /// Not implemented!
            ///
            /// Use `#[routes(with_views, fallback="SomeComponent")] ...`
            /// for this function to be generated.
            pub fn generatedRoutes() -> ! {
                unimplemented!();
            }
        }
    };

    match syn::parse2::<Item>(routes_fn) {
        Ok(routes_fn) => {
            insert_into_module(&mut root_mod, &[], routes_fn);
        }
        Err(e) => return TokenStream::from(e.to_compile_error()),
    }

    let (brace, ref mut content) = match root_mod.content {
        Some((brace, ref mut content)) => (brace, content),
        None => unreachable!("Already checked for empty module"),
    };

    // Reconstruct the module with all additions.
    root_mod.content = Some((brace, content.to_vec()));

    Into::into(quote! { #root_mod })
}

fn generate_routes_component(
    route_infos: &[NestedRouteInfo],
    fallback: Option<ExprWrapper>,
) -> proc_macro2::TokenStream {
    let fallback = fallback.expect("fallback is required").0;

    let mut ts = quote! {};

    fn process_route_info(info: &NestedRouteInfo, ts: &mut proc_macro2::TokenStream) {
        let full_path = &info.full_module_path_to_struct_def();

        if !info.children.is_empty() {
            let layout = info
                .layout
                .as_ref()
                .map(|v| quote! { view=#v })
                .expect("Any #[route] with child routes requires a \"layout\" view! Set an optional \"fallback\" view to handle the immediate path. Remember to embed an `<Outlet />` in your \"layout\" view.`");

            ts.extend([
                quote! {
                    <ParentRoute path=#full_path.path() #layout>
                }
            ]);
            {
                for child in &info.children {
                    process_route_info(child, ts);
                }

                let fallback = info
                    .fallback
                    .as_ref()
                    .map(|v| quote! { view=#v });
                if let Some(fallback) = fallback {
                    ts.extend([
                        quote! {
                            <Route path=::leptos_router::path!("") #fallback/>
                        }
                    ]);
                } else {
                    if info.view.is_some() {
                        panic!("Any #[route] with child routes requires a \"layout\" and an optional \"fallback\". \"view\" must only be set on leaf routes. Replace \"view\" with \"fallback\" or remove the argument.");
                    }
                }
            }
            ts.extend([
                quote! {
                    </ParentRoute>
                }
            ]);
        } else {
            let view = info
                .view
                .as_ref()
                .map(|v| quote! { view=#v })
                .expect("Any leaf #[route] (without children) requires a \"view\"!");

            ts.extend([
                quote! {
                    <Route path=#full_path.path() #view/>
                }
            ]);
        }
    }

    for info in route_infos {
        process_route_info(info, &mut ts);
    }

    quote! {
        pub fn generatedRoutes() -> impl ::leptos::IntoView {
            use ::leptos_router::components::Routes;
            use ::leptos_router::components::ParentRoute;
            use ::leptos_router::components::Route;
            use ::leptos::prelude::*;
            // This allows users to import or define their component in the "mod routes { ... }"
            // surrounding module.
            use super::*;

            view! {
                <Routes fallback=#fallback>
                    #ts
                </Routes>
            }
        }
    }
}

#[proc_macro_attribute]
pub fn route(_attr: TokenStream, input: TokenStream) -> TokenStream {
    input
}

#[derive(Debug, PartialEq, Eq)]
struct NestedRouteInfo {
    /// "/" or "/users"
    path: String,
    layout: Option<Expr>,
    fallback: Option<Expr>,
    view: Option<Expr>,
    /// Pascal-cased name of the module that had this route annotation.
    name: syn::Ident,
    parent_struct: Option<(String, syn::Ident)>,
    vis: Visibility,
    found_in_module_path: Vec<syn::Ident>,
    children: Vec<NestedRouteInfo>,
}

impl NestedRouteInfo {
    fn full_module_path_to_struct_def(&self) -> syn::Path {
        let struct_name = &self.name;
        let paths = &self.found_in_module_path[1..self.found_in_module_path.len() - 1];

        let mut full_path: Option<syn::Path> = paths.iter().next().cloned().map(|it| it.into());
        if full_path.is_some() {
            for next in paths.iter().skip(1) {
                let mut prev = full_path.unwrap();
                prev.segments.push(syn::PathSegment {
                    ident: next.clone(),
                    arguments: PathArguments::None,
                });
                full_path = Some(prev);
            }
        }
        let full_path = full_path
            .map(|mut it| {
                it.segments.push(syn::PathSegment {
                    ident: struct_name.clone(),
                    arguments: PathArguments::None,
                });
                it
            })
            .unwrap_or(struct_name.clone().into());
        full_path
    }
}

fn collect_routes(
    module: &mut ItemMod,
    route_macro_args: RouteMacroArgs,
    parent_path: Option<&str>,
    parent_struct: Option<&syn::Ident>,
    nested_route_infos: &mut Vec<NestedRouteInfo>,
    module_path: Vec<syn::Ident>,
) {
    let module_name = &module.ident;
    let vis = &module.vis;

    // Create current module path
    let mut current_module_path = module_path.clone();
    current_module_path.push(module_name.clone());

    // Add current module's route
    let mut n = NestedRouteInfo {
        path: route_macro_args.route_path_segments.clone(),
        layout: route_macro_args.layout,
        fallback: route_macro_args.fallback,
        view: route_macro_args.view,
        name: format_ident!("{}", to_pascal_case(&module_name.to_string())),
        parent_struct: match (parent_path, parent_struct) {
            (Some(parent_path), Some(parent_struct)) => Some((parent_path.to_owned(), parent_struct.clone())),
            (None, None) => None,
            _ => panic!("Invalid state"), // TODO: phrase
        },
        vis: vis.clone(),
        found_in_module_path: current_module_path.clone(),
        children: Vec::new(),
    };

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
                if let Some(args) = extract_route_attr(&sub_mod.attrs) {
                    collect_routes(
                        sub_mod,
                        args,
                        Some(&route_macro_args.route_path_segments),
                        Some(&current_struct),
                        &mut n.children,
                        current_module_path.clone(),
                    );
                }
            }
        }
    }
    nested_route_infos.push(n);
}

struct RouteMacroArgs {
    /// A path, defined like: "/" or "/users"
    route_path_segments: String,
    /// A wrapper view, defined like: "wrap=MainLayout" or "wrap=|| view! { <MainLayout/> }"
    layout: Option<Expr>,
    fallback: Option<Expr>,
    /// The route view, defined like: "view=SomePage" or "view=|| view! { <SomePage/> }"
    view: Option<Expr>,
}

fn extract_route_attr(attrs: &[Attribute]) -> Option<RouteMacroArgs> {
    attrs
        .iter()
        .find(|attr| attr.path().is_ident("route"))
        .and_then(|attr| {
            attr.parse_args_with(|input: syn::parse::ParseStream| {
                //panic!("Input: {:?}", content);
                let mut path: Option<String> = None;
                let mut layout: Option<Expr> = None;
                let mut fallback: Option<Expr> = None;
                let mut view: Option<Expr> = None;

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
                        } else if ident == "layout" {
                            let _ = input.parse::<syn::Token![=]>()?;
                            let lit = input.parse::<syn::Lit>()?;
                            layout = Some(ExprWrapper::from_value(&lit)?.0);
                        } else if ident == "fallback" {
                            let _ = input.parse::<syn::Token![=]>()?;
                            let lit = input.parse::<syn::Lit>()?;
                            fallback = Some(ExprWrapper::from_value(&lit)?.0);
                        } else {
                            panic!("Unexpected ident: {:?}", ident);
                        }
                    } else {
                        return Err(lookahead.error());
                    }

                    if !input.is_empty() {
                        let _: syn::Token![,] = input.parse()?;
                    }
                }
                let path = path.expect("expect path to be present");
                //if wrap.is_some() {
                //    panic!("Hello: {:?} {:?} {:?}", path, wrap, view);
                //}

                Ok(RouteMacroArgs {
                    route_path_segments: path,
                    layout,
                    fallback,
                    view,
                })
            })
            .ok()
        })
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
    root_route_infos: &[NestedRouteInfo],
    current_info: &NestedRouteInfo,
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

        current = find_parent_of(root_route_infos, info);
    }
    params
}

//fn flatten_one(nested_route_info: &NestedRouteInfo) -> impl Iterator<Item = &NestedRouteInfo> {
//    let mut stack = vec![nested_route_info];
//    from_fn(move || {
//        while let Some(node) = stack.pop() {
//            stack.extend(node.children.as_slice());
//            return Some(node);
//        }
//        None
//    })
//}

fn flatten(root_route_infos: &[NestedRouteInfo]) -> impl Iterator<Item = &NestedRouteInfo> {
    let mut stack = Vec::new();
    stack.extend(root_route_infos);
    from_fn(move || {
        while let Some(node) = stack.pop() {
            stack.extend(node.children.as_slice());
            return Some(node);
        }
        None
    })
}

fn find_parent_of<'a>(
    root_route_infos: &'a [NestedRouteInfo],
    current: &'a NestedRouteInfo,
) -> Option<&'a NestedRouteInfo> {

    fn find_recursive<'a>(
        test: &'a NestedRouteInfo,
        current: &'a NestedRouteInfo,
    ) -> Option<&'a NestedRouteInfo> {
        if test.children.iter().any(|child| child == current) {
            return Some(test);
        }
        for child in &test.children {
            if let Some(parent) = find_recursive(child, current) {
                return Some(parent);
            }
        }
        None
    }

    for info in root_route_infos {
        if let Some(parent) = find_recursive(info, current) {
            return Some(parent);
        }
    }
    None
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
