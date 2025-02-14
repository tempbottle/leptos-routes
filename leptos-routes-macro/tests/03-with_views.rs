use assertr::assert_that;
use assertr::prelude::PartialEqAssertions;
use leptos::prelude::*;
use leptos_router::components::{Outlet, Router};
use leptos_router::location::RequestUrl;
use leptos_router::StaticSegment;
use leptos_routes::routes;

#[routes(with_views, fallback = "|| view! { <FallbackComponent/> }")]
pub mod routes {

    // A route without any segment.
    #[route("/", layout = "MainLayout", fallback = "PageDashboard")]
    pub mod root {

        // A route with a single static segments.
        #[route("/welcome", view = "PageWelcome")]
        pub mod welcome {}

        // A route with multiple static segments.
        #[route("/foo/bar", view = "SomePage")]
        pub mod multiple_static {}

        // A route with multiple segments, not being all static.
        #[route("/foo/:bar", view = "SomePage")]
        pub mod multiple_dynamic {}

        // A route with all types of segments.
        // This route also uses the rust keyword `type` that must be handled.
        #[route("/complex/:foo/:type?/*baz", view = "SomePage")]
        pub mod complex {}

        // Nested routes.
        #[route("/users", layout = "UsersLayout", fallback = "NoUser")]
        pub mod users {

            // The `wrap` attribute on modules containing children is optional!
            #[route("/:id", layout = "UserLayout", fallback="User")]
            pub mod user {

                // This has the same name as a root-level route. That must not lead to a name clash!
                #[route("/settings", view = "UserSettings")]
                pub mod welcome {}

                #[route("/details", view = "UserDetails")]
                pub mod details {}
            }
        }
    }
}

#[component]
fn FallbackComponent() -> impl IntoView {
    view! {
        "Fallback"
    }
}

#[component]
fn MainLayout() -> impl IntoView {
    view! {
        <div id="main-layout">
            <Outlet/>
        </div>
    }
}

#[component]
fn UsersLayout() -> impl IntoView {
    view! {
        <div id="users-layout">
            <Outlet/>
        </div>
    }
}

#[component]
fn UserLayout() -> impl IntoView {
    view! {
        <div id="user-layout">
            <Outlet/>
        </div>
    }
}

#[component]
fn PageDashboard() -> impl IntoView {
    view! {
        "Dashboard"
    }
}

#[component]
fn PageWelcome() -> impl IntoView {
    view! {
        "Welcome"
    }
}

#[component]
fn NoUser() -> impl IntoView {
    view! {
        "NoUser"
    }
}

#[component]
fn User() -> impl IntoView {
    view! {
        "User"
    }
}

#[component]
fn UserSettings() -> impl IntoView {
    view! {
        "UserSettings"
    }
}

#[component]
fn UserDetails() -> impl IntoView {
    view! {
        "UserDetails"
    }
}

#[component]
fn SomePage() -> impl IntoView {
    view! {
        "SomePage"
    }
}

fn main() {
    fn app() -> impl IntoView {
        view! {
            <Router>
                { routes::generatedRoutes() }
            </Router>
        }
    }

    let _ = Owner::new_root(None);

    assert_that(routes::Root.path()).is_equal_to(());
    assert_that(routes::Root.materialize()).is_equal_to("/");

    assert_that(routes::root::Welcome.path()).is_equal_to((StaticSegment("welcome"),));
    assert_that(routes::root::Welcome.materialize()).is_equal_to("/welcome");

    provide_context::<RequestUrl>(RequestUrl::default());
    assert_that(app().to_html()).is_equal_to(r#"<div id="main-layout">Dashboard</div>"#);

    provide_context::<RequestUrl>(RequestUrl::new(
        routes::root::Welcome.materialize().as_str(),
    ));
    assert_that(app().to_html()).is_equal_to(r#"<div id="main-layout">Welcome</div>"#);

    provide_context::<RequestUrl>(RequestUrl::new(
        routes::root::users::user::Details
            .materialize("42")
            .as_str(),
    ));
    assert_that(app().to_html()).is_equal_to(r#"<div id="main-layout"><div id="users-layout"><div id="user-layout">UserDetails</div></div></div>"#);
}
