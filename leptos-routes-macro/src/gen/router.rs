use crate::route_def::RouteDef;
use crate::ExprWrapper;
use proc_macro_error2::abort;
use quote::quote;

pub fn generate_routes_component(
    route_defs: &[RouteDef],
    fallback: Option<ExprWrapper>,
) -> proc_macro2::TokenStream {
    let fallback = fallback.expect("fallback is required").0;

    let mut ts = quote! {};

    fn process_route_def(route_def: &RouteDef, ts: &mut proc_macro2::TokenStream) {
        let full_path = &route_def.full_module_path_to_struct_def();

        if !route_def.children.is_empty() {
            let layout = route_def
                .layout
                .as_ref()
                .map(|v| quote! { view=#v })
                .unwrap_or_else(|| abort! {
                    route_def.route_ident_span,
                    "Any #[route] with child routes requires a \"layout\" view! Set an optional \"fallback\" view to handle the immediate path. Remember to embed an `<Outlet />` in your \"layout\" view.`"
                });

            ts.extend([quote! {
                <ParentRoute path=#full_path.path() #layout>
            }]);
            {
                for child in &route_def.children {
                    process_route_def(child, ts);
                }

                let fallback = route_def.fallback.as_ref().map(|v| quote! { view=#v });
                if let Some(fallback) = fallback {
                    ts.extend([quote! {
                        <Route path=::leptos_router::path!("") #fallback/>
                    }]);
                } else if route_def.view.is_some() {
                    abort!(
                        route_def.view_span.expect("present"),
                        "Any #[route] with child routes requires a \"layout\" and an optional \"fallback\". \"view\" must only be set on leaf routes. Replace \"view\" with \"fallback\" or remove the argument."
                    );
                }
            }
            ts.extend([quote! {
                </ParentRoute>
            }]);
        } else {
            let view = route_def
                .view
                .as_ref()
                .map(|v| quote! { view=#v })
                .unwrap_or_else(|| {
                    abort! {
                        route_def.route_ident_span,
                        "Any leaf #[route] (without children) requires a \"view\"!"
                    }
                });

            ts.extend([quote! {
                <Route path=#full_path.path() #view/>
            }]);
        }
    }

    for route_def in route_defs {
        process_route_def(route_def, &mut ts);
    }

    quote! {
        pub fn generated_routes() -> impl ::leptos::IntoView {
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
