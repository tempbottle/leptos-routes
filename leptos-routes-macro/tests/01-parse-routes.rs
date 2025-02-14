use leptos_routes::routes;

#[routes]
pub mod routes {

    // A route without any segment.
    #[route("/")]
    pub mod root {

        // A route with a single static segments.
        #[route("/welcome")]
        pub mod welcome {}

        // A route with multiple static segments.
        #[route("/foo/bar")]
        pub mod multiple_static {}

        // A route with multiple segments, not being all static.
        #[route("/foo/:bar")]
        pub mod multiple_dynamic {}

        // A route with all types of segments.
        // This route also uses the rust keyword `type` that must be handled.
        #[route("/complex/:foo/:type?/*baz")]
        pub mod complex {}

        // Nested routes.
        #[route("/users")]
        pub mod users {

            #[route("/:id")]
            pub mod user {

                // This has the same name as a root-level route. That must not lead to a name clash!
                #[route("/welcome")]
                pub mod welcome {}

                #[route("/details")]
                pub mod details {}
            }
        }
    }
}

fn main() {
    use assertr::prelude::*;
    use leptos_router::{OptionalParamSegment, ParamSegment, StaticSegment, WildcardSegment};

    assert_that(routes::Root.path()).is_equal_to(());
    assert_that(routes::Root.materialize()).is_equal_to("/");

    assert_that(routes::root::Welcome.path()).is_equal_to((StaticSegment("welcome"),));
    assert_that(routes::root::Welcome.materialize()).is_equal_to("/welcome");

    assert_that(routes::root::MultipleStatic.path())
        .is_equal_to((StaticSegment("foo"), StaticSegment("bar")));
    assert_that(routes::root::MultipleStatic.materialize()).is_equal_to("/foo/bar");

    assert_that(routes::root::MultipleDynamic.path())
        .is_equal_to((StaticSegment("foo"), ParamSegment("bar")));
    assert_that(routes::root::MultipleDynamic.materialize("some-value")).is_equal_to("/foo/some-value");

    assert_that(routes::root::Complex.path()).is_equal_to((
        StaticSegment("complex"),
        ParamSegment("foo"),
        OptionalParamSegment("type"),
        WildcardSegment("baz"),
    ));
    assert_that(routes::root::Complex.materialize("42", Some("ok"), "bob"))
        .is_equal_to("/complex/42/ok/bob");
    assert_that(routes::root::Complex.materialize("42", None, "otto")).is_equal_to("/complex/42/otto");

    assert_that(routes::root::Users.path()).is_equal_to((StaticSegment("users"),));
    assert_that(routes::root::Users.materialize()).is_equal_to("/users");

    assert_that(routes::root::users::User.path()).is_equal_to((ParamSegment("id"),));
    assert_that(routes::root::users::User.materialize("42")).is_equal_to("/users/42");

    assert_that(routes::root::users::user::Details.path()).is_equal_to((StaticSegment("details"),));
    assert_that(routes::root::users::user::Details.materialize("42")).is_equal_to("/users/42/details");

    // Routes can be checked for equality
    assert_that(routes::Root).is_equal_to(routes::Root);

    // A `Route` enum is generated which allows referring to "any route" using a variant.
    // This has limited usability though, as both `path()` and `materialize()` of the contained
    // have structs have no common type-signature.
    let route: routes::Route = routes::Route::RootUsersUserDetails(routes::root::users::user::Details);
    match route {
        routes::Route::Root(_route) => {}
        routes::Route::RootWelcome(_) => {}
        routes::Route::RootMultipleStatic(_) => {}
        routes::Route::RootMultipleDynamic(_) => {}
        routes::Route::RootComplex(_) => {}
        routes::Route::RootUsers(_) => {}
        routes::Route::RootUsersUser(_) => {}
        routes::Route::RootUsersUserWelcome(_) => {}
        routes::Route::RootUsersUserDetails(_) => {}
    }
}
