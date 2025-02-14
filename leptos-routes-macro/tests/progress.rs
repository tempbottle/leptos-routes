#[test]
fn tests() {
    let t = trybuild::TestCases::new();
    t.pass("tests/01-parse-routes.rs");
    t.pass("tests/02-without_views_not_router_generation.rs");
    t.pass("tests/03-with_views.rs");
}
