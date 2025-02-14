use assertr::assert_that;
use assertr::prelude::PartialEqAssertions;
use leptos::prelude::*;
use leptos_router::components::{Outlet, Router};
use leptos_router::location::RequestUrl;
use leptos_routes::routes;

#[routes(with_views, fallback = "|| view! { <Err404/> }")]
pub mod routes {

    #[route("/", layout = "MainLayout", fallback = "Dashboard")]
    pub mod root {

        #[route("/welcome", view = "Welcome")]
        pub mod welcome {}

        #[route("/users", layout = "UsersLayout", fallback = "NoUser")]
        pub mod users {

            #[route("/:id", layout = "UserLayout", fallback="User")]
            pub mod user {

                #[route("/details", view = "UserDetails")]
                pub mod details {}
            }
        }
    }
}

#[component]
fn Err404() -> impl IntoView { view! { "Err404" } }
#[component]
fn MainLayout() -> impl IntoView { view! { <div id="main-layout"> <Outlet/> </div> } }
#[component]
fn UsersLayout() -> impl IntoView { view! { <div id="users-layout"> <Outlet/> </div> } }
#[component]
fn UserLayout() -> impl IntoView { view! { <div id="user-layout"> <Outlet/> </div> } }
#[component]
fn Dashboard() -> impl IntoView { view! { "Dashboard" } }
#[component]
fn Welcome() -> impl IntoView { view! { "Welcome" } }
#[component]
fn NoUser() -> impl IntoView { view! { "NoUser" } }
#[component]
fn User() -> impl IntoView { view! {"User" } }
#[component]
fn UserDetails() -> impl IntoView { view! { "UserDetails" } }

fn main() {
    fn app() -> impl IntoView {
        view! {
            <Router>
                { routes::generated_routes() }
            </Router>
        }
    }

    let _ = Owner::new_root(None);

    provide_context::<RequestUrl>(RequestUrl::new(
        routes::root::users::user::Details
            .materialize("42")
            .as_str(),
    ));
    assert_that(app().to_html()).is_equal_to(r#"<div id="main-layout"><div id="users-layout"><div id="user-layout">UserDetails</div></div></div>"#);
}
