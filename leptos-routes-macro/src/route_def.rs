use crate::path::PathSegments;
use crate::route_macro_args::RouteMacroArgs;
use crate::util::to_pascal_case;
use crate::ModulePath;
use proc_macro2::Span;
use quote::format_ident;
use std::iter::from_fn;
use syn::spanned::Spanned;
use syn::{Expr, Item, ItemMod, PathArguments, Visibility};
use uuid::Uuid;

#[derive(Debug)]
pub struct RouteDef {
    /// Any observed route will get a unique, random identifier.
    /// Using this identifier, we can omit an equality implementation on this type.
    pub id: Uuid,

    #[expect(unused)]
    pub module_span: Span,
    pub route_ident_span: Span,

    /// "/" or "/users"
    pub path: String,
    pub path_segments: PathSegments,

    pub layout: Option<Expr>,
    #[expect(unused)]
    pub layout_span: Option<Span>,

    pub fallback: Option<Expr>,
    #[expect(unused)]
    pub fallback_span: Option<Span>,

    pub view: Option<Expr>,
    pub view_span: Option<Span>,

    /// Pascal-cased name of the module that had this route annotation.
    pub name: syn::Ident,
    pub parent_struct: Option<(String, syn::Ident)>,
    pub vis: Visibility,
    pub found_in_module_path: ModulePath,
    pub children: Vec<RouteDef>,
}

impl RouteDef {
    pub fn full_module_path_to_struct_def(&self) -> syn::Path {
        let struct_name = &self.name;
        let paths = &self.found_in_module_path.without_first();

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
        full_path
            .map(|mut it| {
                it.segments.push(syn::PathSegment {
                    ident: struct_name.clone(),
                    arguments: PathArguments::None,
                });
                it
            })
            .unwrap_or(struct_name.clone().into())
    }
}

pub fn collect_route_definitions(
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
        id: Uuid::new_v4(),
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

pub fn flatten(root_route_defs: &[RouteDef]) -> impl Iterator<Item = &RouteDef> {
    let mut stack = Vec::new();
    stack.extend(root_route_defs);
    from_fn(move || {
        if let Some(node) = stack.pop() {
            stack.extend(node.children.as_slice());
            return Some(node);
        }
        None
    })
}

pub fn find_parent_of<'a>(
    root_route_defs: &'a [RouteDef],
    current: &'a RouteDef,
) -> Option<&'a RouteDef> {
    fn find_recursive<'a>(test: &'a RouteDef, current: &'a RouteDef) -> Option<&'a RouteDef> {
        if test.children.iter().any(|child| child.id == current.id) {
            return Some(test);
        }
        for child in &test.children {
            if let Some(parent) = find_recursive(child, current) {
                return Some(parent);
            }
        }
        None
    }

    for route_def in root_route_defs {
        if let Some(parent) = find_recursive(route_def, current) {
            return Some(parent);
        }
    }
    None
}
