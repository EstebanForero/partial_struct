use partial_struct::Partial;

#[derive(Partial, Debug, PartialEq)]
#[partial(derive(Debug, PartialEq), omit(id), optional(email))]
struct User {
    id: u32,
    name: String,
    email: String,
}

#[test]
fn split_and_rebuild_with_optional() {
    let full = User {
        id: 7,
        name: "Ada".to_string(),
        email: "ada@example.com".to_string(),
    };

    let (partial, omitted) = PartialUser::from_user_with_omitted(full);
    assert_eq!(partial.name, "Ada");
    assert_eq!(partial.email.as_deref(), Some("ada@example.com"));
    assert_eq!(omitted.id, 7);

    let rebuilt = partial.to_user(omitted.id, None);
    assert_eq!(
        rebuilt,
        User {
            id: 7,
            name: "Ada".to_string(),
            email: "ada@example.com".to_string(),
        }
    );
}

#[test]
fn full_into_partial_with_omitted() {
    let full = User {
        id: 11,
        name: "Lin".to_string(),
        email: "lin@example.com".to_string(),
    };

    let (partial, omitted) = full.into_partial_user_with_omitted();
    assert_eq!(partial.name, "Lin");
    assert_eq!(partial.email.as_deref(), Some("lin@example.com"));
    assert_eq!(omitted.id, 11);
}

#[derive(Partial, Debug, PartialEq)]
#[partial(derive(Debug, PartialEq))]
struct Point {
    x: i32,
    y: i32,
}

#[test]
fn split_without_omitted_fields() {
    let full = Point { x: 1, y: 2 };
    let (partial, omitted) = PartialPoint::from_point_with_omitted(full);
    assert_eq!(partial, PartialPoint { x: 1, y: 2 });
    assert_eq!(omitted, ());

    let full = Point { x: 3, y: 4 };
    let (partial, omitted) = full.into_partial_point_with_omitted();
    assert_eq!(partial, PartialPoint { x: 3, y: 4 });
    assert_eq!(omitted, ());
}

#[derive(Partial, Debug, PartialEq)]
#[partial(derive(Debug, PartialEq), omit(a, b), optional(c))]
struct MultiOmit {
    a: u8,
    b: u8,
    c: u8,
    d: u8,
}

#[test]
fn split_with_multiple_omitted_fields() {
    let full = MultiOmit {
        a: 1,
        b: 2,
        c: 3,
        d: 4,
    };

    let (partial, omitted) = PartialMultiOmit::from_multi_omit_with_omitted(full);
    assert_eq!(partial.d, 4);
    assert_eq!(partial.c, Some(3));
    assert_eq!(omitted.a, 1);
    assert_eq!(omitted.b, 2);
}
