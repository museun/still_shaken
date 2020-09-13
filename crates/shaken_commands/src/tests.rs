use super::*;

#[test]
fn parse_good() {
    let tests = vec![
        "!foo <req> <opt?>",
        "!foo <req> <opt?> <opt2?>",
        "!foo <req> <opt?> <flex...>",
        "!foo <req> <flex...>",
        "!foo <flex...>",
        "!foo <opt?> <flex...>",
        "!foo <opt?> <opt2?> <flex...>",
    ];

    for test in tests {
        Command::example(test).build().unwrap();
    }
}

#[test]
fn display() {
    let tests = vec![
        "!foo <req> <opt?>",
        "!foo <req> <opt?> <opt2?>",
        "!foo <req> <opt?> <flex...>",
        "!foo <req> <flex...>",
        "!foo <flex...>",
        "!foo <opt?> <flex...>",
        "!foo <opt?> <opt2?> <flex...>",
    ];

    for test in tests {
        assert_eq!(Command::example(test).build().unwrap().to_string(), test);
    }
}

#[test]
fn parse_failure() {
    let tests = vec![
        "!foo <opt?> <req>",
        "!foo <flex...> <opt?>",
        "!foo <flex...> <req>",
        "!foo <req> <opt?> <req2>",
        "!foo <dup> <opt?> <dup>",
        "!foo <flex1...> <flex2...>",
        "!foo <opt?> <flex1...> <flex2...>",
        "!foo <req> <opt?> <flex1...> <flex2...>",
    ];

    for test in tests {
        Command::example(test).build().unwrap_err();
    }
}

#[test]
fn matched() {
    use ExtractResult::*;

    let cmd = Command::example("!hello <name> <other?> <rest...>")
        .build()
        .unwrap();

    for input in &["!hello world this is a test", "!hello world"] {
        assert!(matches!(cmd.extract(input), Found(map) if !map.is_empty()));
    }

    assert!(matches!(cmd.extract("!hello"), Required));

    for input in &["!testing world this is a test", "!", ""] {
        assert!(matches!(cmd.extract(*input), NoMatch))
    }
}

#[test]
fn extract() {
    use ExtractResult::*;

    let cmd = Command::example("!hello <name> <other>").build().unwrap();
    let map = match cmd.extract("!hello world testing this") {
        Found(map) => map,
        _ => panic!(),
    };

    assert_eq!(map["name"], "world");
    assert_eq!(map["other"], "testing");

    let map = match cmd.extract("!hello world testing") {
        Found(map) => map,
        _ => panic!(),
    };
    assert_eq!(map["name"], "world");
    assert_eq!(map["other"], "testing");

    let cmd = Command::example("!hello <name> <other> <tail...>")
        .build()
        .unwrap();

    let map = match cmd.extract("!hello world testing this is the tail") {
        Found(map) => map,
        _ => panic!(),
    };
    assert_eq!(map["name"], "world");
    assert_eq!(map["other"], "testing");
    assert_eq!(map["tail"], "this is the tail");

    let map = match cmd.extract("!hello world testing") {
        Found(map) => map,
        _ => panic!(),
    };
    assert_eq!(map["name"], "world");
    assert_eq!(map["other"], "testing");
}
