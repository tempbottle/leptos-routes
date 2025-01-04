use assertr::prelude::*;
use leptos_router::{OptionalParamSegment, ParamSegment, StaticSegment, WildcardSegment};
use leptos_routes::routes;

#[routes]
pub mod routes {

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

            #[route("/details")]
            pub mod details {}
        }
    }
}

fn main() {
    assert_that(routes::Welcome.path()).is_equal_to((StaticSegment("welcome"),));
    assert_that(routes::Welcome.materialize()).is_equal_to("/welcome");

    assert_that(routes::MultipleStatic.path())
        .is_equal_to((StaticSegment("foo"), StaticSegment("bar")));
    assert_that(routes::MultipleStatic.materialize()).is_equal_to("/foo/bar");

    assert_that(routes::MultipleDynamic.path())
        .is_equal_to((StaticSegment("foo"), ParamSegment("bar")));
    assert_that(routes::MultipleDynamic.materialize("some-value")).is_equal_to("/foo/some-value");

    assert_that(routes::Complex.path()).is_equal_to((
        StaticSegment("complex"),
        ParamSegment("foo"),
        OptionalParamSegment("type"),
        WildcardSegment("baz"),
    ));
    assert_that(routes::Complex.materialize("42", Some("ok"), "bob"))
        .is_equal_to("/complex/42/ok/bob");
    assert_that(routes::Complex.materialize("42", None, "otto")).is_equal_to("/complex/42/otto");

    assert_that(routes::Users.path()).is_equal_to((StaticSegment("users"),));
    assert_that(routes::Users.materialize()).is_equal_to("/users");

    assert_that(routes::users::User.path()).is_equal_to((ParamSegment("id"),));
    assert_that(routes::users::User.materialize("42")).is_equal_to("/users/42");

    assert_that(routes::users::user::Details.path()).is_equal_to((StaticSegment("details"),));
    assert_that(routes::users::user::Details.materialize("42")).is_equal_to("/users/42/details");
}
