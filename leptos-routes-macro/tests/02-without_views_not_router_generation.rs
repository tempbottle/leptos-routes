use assertr::assert_that_panic_by;
use assertr::prelude::{PanicValueAssertions, PartialEqAssertions};
use leptos_routes::routes;

#[routes]
pub mod routes {

    #[route("/")]
    pub mod root {}
}

fn main() {
    // Assumption: `generatedRoutes` is generated but immediately panics using `unimplemented!`.
    assert_that_panic_by(|| {
        let _never = routes::generatedRoutes();
    })
    .has_type::<&str>()
    .is_equal_to("not implemented");
}
