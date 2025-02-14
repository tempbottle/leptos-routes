use crate::route_def::{find_parent_of, RouteDef};
use quote::quote;

#[derive(Debug, Clone)]
pub struct ParamInfo {
    pub name: String,
    pub is_optional: bool,
    #[expect(unused)]
    pub is_wildcard: bool,
}

impl ParamInfo {
    /// Collect parameters from a route and its parents.
    pub fn collect_params_through_hierarchy(
        root_route_defs: &[RouteDef],
        current_route: &RouteDef,
    ) -> Vec<ParamInfo> {
        let mut params = Vec::new();
        let mut current = Some(current_route);

        while let Some(route_def) = current {
            for seg in &route_def.path_segments.segments {
                match seg {
                    PathSegment::Param(name) => params.push(ParamInfo {
                        name: name.clone(),
                        is_optional: false,
                        is_wildcard: false,
                    }),
                    PathSegment::OptionalParam(name) => params.push(ParamInfo {
                        name: name.clone(),
                        is_optional: true,
                        is_wildcard: false,
                    }),
                    PathSegment::Wildcard(name) => params.push(ParamInfo {
                        name: name.clone(),
                        is_optional: false,
                        is_wildcard: true,
                    }),
                    PathSegment::Static(_) => {}
                }
            }

            current = find_parent_of(root_route_defs, route_def);
        }
        params
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum PathSegment {
    Static(String),
    Param(String),
    OptionalParam(String),
    Wildcard(String),
}

#[derive(Debug, PartialEq, Eq)]
pub struct PathSegments {
    pub segments: Vec<PathSegment>,
}

impl PathSegments {
    pub fn parse(path: &str) -> PathSegments {
        let segments = path
            .split('/')
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
            .collect();
        PathSegments { segments }
    }

    /// Generates the appropriate tuple-type for these segments.
    pub fn generate_path_type(&self) -> proc_macro2::TokenStream {
        let segment_types = self.segments.iter().map(|segment| match segment {
            PathSegment::Static(_) => quote!(::leptos_router::StaticSegment<&'static str>),
            PathSegment::Param(_) => quote!(::leptos_router::ParamSegment),
            PathSegment::OptionalParam(_) => quote!(::leptos_router::OptionalParamSegment),
            PathSegment::Wildcard(_) => quote!(::leptos_router::WildcardSegment),
        });

        match self.segments.len() {
            0 => quote!(()),
            _ => quote!((#(#segment_types,)*)),
        }
    }
}
