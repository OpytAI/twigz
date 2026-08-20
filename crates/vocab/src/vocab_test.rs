use twigz_vocab::{kind_by_name, Kind, GRAMMAR_IR_VERSION, SEMANTIC_KINDS, VOCABULARY_VERSION};

#[test]
fn vocabulary_version_is_two() {
    assert_eq!(VOCABULARY_VERSION, 2);
    assert_eq!(GRAMMAR_IR_VERSION, 2);
}

#[test]
fn kinds_one_through_twenty_two_are_locked() {
    let expected = [
        (1, "module"),
        (2, "declaration"),
        (3, "function"),
        (4, "parameter"),
        (5, "call"),
        (6, "member"),
        (7, "identifier"),
        (8, "literal"),
        (9, "type"),
        (10, "block"),
        (11, "assignment"),
        (12, "branch"),
        (13, "loop"),
        (14, "return"),
        (15, "import"),
        (16, "table"),
        (17, "field"),
        (18, "operator"),
        (19, "comment"),
        (20, "class"),
        (21, "namespace"),
        (22, "string"),
    ];
    assert_eq!(SEMANTIC_KINDS.len(), expected.len());
    for (index, (id, name)) in expected.iter().enumerate() {
        assert_eq!(SEMANTIC_KINDS[index].id, *id, "{name}");
        assert_eq!(SEMANTIC_KINDS[index].name, *name);
        assert_eq!(kind_by_name(name).map(|spec| spec.id), Some(*id));
    }
    assert_eq!(Kind::FUNCTION.0, 3);
    assert_eq!(Kind::CLASS.0, 20);
    assert_eq!(Kind::STRING.0, 22);
    assert_eq!(Kind::from_name("class"), Some(Kind::CLASS));
    assert!(kind_by_name("protocol").is_none());
}

#[test]
fn class_namespace_and_string_roles() {
    let class = kind_by_name("class").unwrap();
    assert!(class
        .roles
        .iter()
        .any(|role| role.name == "name" && role.required));
    assert!(class
        .roles
        .iter()
        .any(|role| role.name == "body" && role.required));
    assert!(class.traits.iter().any(|item| item.name == "declaration"));
    assert!(class.traits.iter().any(|item| item.name == "scope"));
    let namespace = kind_by_name("namespace").unwrap();
    assert!(namespace
        .roles
        .iter()
        .any(|role| role.name == "name" && !role.required));
    assert!(namespace
        .roles
        .iter()
        .any(|role| role.name == "body" && role.required));
    let string = kind_by_name("string").unwrap();
    assert!(string.roles.is_empty());
    assert!(string.traits.is_empty());
}
