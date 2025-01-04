# leptos-routes

Declaratively define the routes for your Leptos project.

## Example

```rust
use leptos_routes::routes;

#[routes]
pub mod routes {
  
    #[route("/users")]
    pub mod users {
  
        #[route("/:id")]
        pub mod user {
  
            #[route("/details")]
            pub mod details {}
        }
    }
}
```

## What does it do?

The `routes` proc-macro parses the module hierarchy and generates a struct for each individual route in your
application. You can access these routes through your `routes` and any of its submodules. The structure will be kept.

```rust
let users_route = routes::Users;
let details_route = routes::users::user::Details;
```

Each of these structs implements the following function:

- `path() -> Segments`, where `Segments` is a dynamically sized tuple based on the amount of segments present in the
  path passed to `route`. This only returns the segments declared on the mod itself. Not all its prent segments. This
  makes this usable in places where you otherwise would have used the`path!` macro from `leptos_router`, for example
  when declaring your `<Router>`!
  ```rust
  use assertr::prelude::*;
  assert_that(routes::users::User.path()).is_equal_to((ParamSegment("id"),));
  ```
  or anywhere else where some kind of `Segments` are required.

- `materialize(...) -> String` materializes a usable URL from the full path (including all parent segments) usable in,
  for example, links to parts of your application. As String is `IntoHref`, the return value can be used anywhere an
  `IntoHref` is required.
  ```rust
  use assertr::prelude::*;
  assert_that(routes::users::user::Details.materialize("42")).is_equal_to("/users/42/details");
  ```

## Testing

Run all tests of all creates using the `--all` flag when in the root directory:

    cargo test --all
