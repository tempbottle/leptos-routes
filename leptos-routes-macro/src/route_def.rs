use crate::path::PathSegments;
use crate::ModulePath;
use std::iter::from_fn;
use proc_macro2::Span;
use syn::{Expr, PathArguments, Visibility};
use uuid::Uuid;

#[derive(Debug)]
pub struct RouteDef {
    /// Any observed routes will get a unique, random identifier.
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

pub fn flatten(root_route_defs: &[RouteDef]) -> impl Iterator<Item = &RouteDef> {
    let mut stack = Vec::new();
    stack.extend(root_route_defs);
    from_fn(move || {
        while let Some(node) = stack.pop() {
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
