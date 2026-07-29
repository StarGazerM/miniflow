#![allow(clippy::cast_possible_truncation)]

use ascent_flow::ascent_flow;

ascent_flow! {
    struct Percentile;
    relation seed(i32);
    relation foo(i32, i32);
    relation bar(i32, i32, i32);
    relation less_or_equal(i32, i32, i32);
    relation rank(i32, i32, usize);
    relation baz(i32, i32);

    seed(0);
    foo(1, 2);
    foo(10, 11);
    bar(1, x, x * 2),
    bar(10, x * 10, x * 20),
    bar(100, x * 100, x * 200) <-- seed(_), for x in 1..100;

    less_or_equal(group, candidate, value) <--
        bar(group, candidate, _),
        bar(group, value, _),
        if value <= candidate;
    rank(group, candidate, count) <--
        bar(group, candidate, _),
        agg count = count() in less_or_equal(group, candidate, _);
    baz(group, candidate) <-- foo(group, _), rank(group, candidate, 75);
}

ascent_flow! {
    struct GroupedMeans;
    relation seed(i32);
    relation foo(i32, i32);
    relation bar(i32, i32, i32);
    relation baz(i32, i32, i32);

    seed(0);
    foo(1, 2);
    foo(10, 11);
    bar(1, x, x * 2),
    bar(10, x * 10, x * 20),
    bar(100, x * 100, x * 200) <-- seed(_), for x in 1..100;
    baz(group, x_mean.round() as i32, y_mean.round() as i32) <--
        foo(group, _),
        agg x_mean = mean(x) in bar(group, x, _),
        agg y_mean = mean(y) in bar(group, _, y);
}

ascent_flow! {
    struct NegationWithWildcard;
    relation foo(i32, i32);
    relation bar(i32, i32, i32);
    relation baz(i32, i32);
    relation baz2(i32, i32);

    foo(0, 1);
    foo(1, 2);
    foo(10, 11);
    foo(100, 101);
    bar(1, 2, 102);
    bar(10, 11, 20);
    bar(10, 11, 12);
    baz(x, y) <-- foo(x, y), !bar(x, y, _);
    baz2(x, y) <-- foo(x, y), !bar(x, y, _);
}

ascent_flow! {
    struct NegationExact;
    relation foo(i32, i32);
    relation bar(i32, i32);
    relation baz(i32, i32);

    foo(0, 1);
    foo(1, 2);
    foo(10, 11);
    foo(100, 101);
    bar(1, 2);
    bar(10, 11);
    bar(10, 11);
    baz(x, y) <-- foo(x, y), !bar(x, y);
}

ascent_flow! {
    struct NegationExpression;
    relation foo(i32, i32);
    relation bar(i32, i32, i32);
    relation baz(i32, i32);

    foo(0, 1);
    foo(1, 2);
    foo(10, 11);
    foo(100, 101);
    bar(1, 2, 3);
    bar(10, 11, 13);
    baz(x, y) <-- foo(x, y), !bar(x, y, y + 1);
}

ascent_flow! {
    struct SimpleMean;
    relation foo(i32);
    relation bar(i32);
    foo(0);
    foo(10);
    bar(mean.round() as i32) <-- agg mean = mean(x) in foo(x);
}

pub fn check() {
    let mut percentile = Percentile::default();
    percentile.run();
    assert_eq!(percentile.baz, vec![(1, 75), (10, 750)]);

    let mut means = GroupedMeans::default();
    means.run();
    assert_eq!(means.baz, vec![(1, 50, 100), (10, 500, 1000)]);

    let expected = vec![(0, 1), (100, 101)];
    let mut wildcard = NegationWithWildcard::default();
    wildcard.run();
    assert_eq!(wildcard.baz, expected);
    assert_eq!(wildcard.baz2, expected);

    let mut exact = NegationExact::default();
    exact.run();
    assert_eq!(exact.baz, vec![(0, 1), (100, 101)]);

    let mut expression = NegationExpression::default();
    expression.run();
    assert_eq!(expression.baz, vec![(0, 1), (10, 11), (100, 101)]);

    let mut mean = SimpleMean::default();
    mean.run();
    assert_eq!(mean.bar, vec![(5,)]);
}
