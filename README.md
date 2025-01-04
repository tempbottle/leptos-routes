# leptos-routes

Declaratively define the routes for your Leptos project.

## Example

```rust
use leptos_routes::routes;

#[routes]
pub mod routes {
    #[route("/")]
    pub mod root {}

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
application. You can access these routes through your `routes` module and any of its submodules.

The example above will create the following structs (the module names determine the struct names, the module hierarchy
will be kept.):

```rust
let _ = routes::Root;
let _ = routes::Users;
let _ = routes::users::User;
let _ = routes::users::user::Details;
```

Each of these structs implements the following functions:

- `path() -> Segments`, where `Segments` is a dynamically sized tuple based on the amount of segments present in the
  path passed to `route`. This only returns the segments declared on the currently evaluated `mod` itself and does not
  also include all of its prent segments.

  This makes `path` usable in `<Route>` declarations, where you otherwise would have directly used the `path!` macro
  from `leptos_router`, or anywhere else where some kind of `Segments` are required.
  ```rust
  use assertr::prelude::*;
  assert_that(routes::users::User.path()).is_equal_to((ParamSegment("id"),));
  ```

- `materialize(...) -> String` materializes a usable URL path from the full list of path-segments, including all parent
  segments.

  Use it to create links to parts of your application. As `IntoHref` is implemented for `String`, the return value can
  be used anywhere an `IntoHref` is required, for example Leptos's `<A>` component!

  This function is automatically generated to take all necessary user-inputs in order to replace
  all dynamic path segments with concrete values, meaning that `materialize` might take 0-n inputs when the full path
  has n segments.
  ```rust
  use assertr::prelude::*;
  assert_that(routes::users::user::Details.materialize("42")).is_equal_to("/users/42/details");
  ```

## Motivation

Having this router declaration

```
<Router>
  <Routes>
    <Route path=path!("/") view=Home/>
    <ParentRoute path=path!("/users") view=Users/>
      <ParentRoute path=path!("/:id") view=User/>
        <Route path=path!("/details") view=Details/>
      </ParentRoute>
    </ParentRoute>
  </Routes>
</Router>
```

Leaves us with perfectly functional routing, but with no way to **refer** to any route.

In many parts of our application, we might want to link to some other internal place.
We would have to write these links by hand, leaving all possible compile-time checks on the table.

I wanted some constant value representing a route which can be used both in `<Route>` declarations as well as in `<a>`
links or any other place consuming some `Segments` or anything `ToHref`.

Creating constants for the values returned by the `path` macro is cumbersome because of the dynamic types and because
nested route declarations only need the additional path segments specified, these constants would be meaningless without
establishing some parent associations. Only then would we be able to format a full link.

Materializing a link from a list of path segments also requires replacing any dynamic/placeholder segments with concrete
values. The theoretically unlimited number of combinations of segments makes this hard to implement as a trait function.

Therefore, auto-generating structs for your routes, and materialization-function only for the combinations of segments
used seemed alright.

With the above/initially presented `routes` module, you can write the router declaration as

```
<Router>
  <Routes>
    <Route path=routes::Root.path() view=Home/>
    <ParentRoute path=routes::Users.path() view=Users/>
      <ParentRoute path=routes:users::User.path() view=User/>
        <Route path=routes::users::user::Details.path() view=Details/>
      </ParentRoute>
    </ParentRoute>
  </Routes>
</Router>
```

and also create links with the same structs as in

```
<a href=routes::users::user::Details.materialize("42")>
  "User 42"
</a>
```

## Testing

Run all tests of all creates using the `--all` flag when in the root directory:

    cargo test --all
