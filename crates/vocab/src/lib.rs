//! Semantic vocabulary kinds 1–22.

use std::collections::BTreeMap;

pub const VOCABULARY_VERSION: u32 = 2;
pub const GRAMMAR_IR_VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Kind(pub u32);

impl Kind {
    pub const MODULE: Self = Self(1);
    pub const DECLARATION: Self = Self(2);
    pub const FUNCTION: Self = Self(3);
    pub const PARAMETER: Self = Self(4);
    pub const CALL: Self = Self(5);
    pub const MEMBER: Self = Self(6);
    pub const IDENTIFIER: Self = Self(7);
    pub const LITERAL: Self = Self(8);
    pub const TYPE: Self = Self(9);
    pub const BLOCK: Self = Self(10);
    pub const ASSIGNMENT: Self = Self(11);
    pub const BRANCH: Self = Self(12);
    pub const LOOP: Self = Self(13);
    pub const RETURN: Self = Self(14);
    pub const IMPORT: Self = Self(15);
    pub const TABLE: Self = Self(16);
    pub const FIELD: Self = Self(17);
    pub const OPERATOR: Self = Self(18);
    pub const COMMENT: Self = Self(19);
    pub const CLASS: Self = Self(20);
    pub const NAMESPACE: Self = Self(21);
    pub const STRING: Self = Self(22);

    pub fn from_name(name: &str) -> Option<Self> {
        SEMANTIC_KINDS
            .iter()
            .find(|spec| spec.name == name)
            .map(|spec| Self(spec.id))
    }

    pub fn name(self) -> Option<&'static str> {
        SEMANTIC_KINDS
            .iter()
            .find(|spec| spec.id == self.0)
            .map(|spec| spec.name)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Role(pub u32);

impl Role {
    pub const NAME: Self = Self(1);
    pub const BODY: Self = Self(2);
    pub const PARAMETERS: Self = Self(3);
    pub const RECEIVER: Self = Self(4);
    pub const ARGUMENTS: Self = Self(5);
    pub const CALLEE: Self = Self(6);
    pub const LEFT: Self = Self(7);
    pub const RIGHT: Self = Self(8);
    pub const CONDITION: Self = Self(9);
    pub const RETURN_TYPE: Self = Self(10);
    pub const VALUE: Self = Self(11);
    pub const SOURCE: Self = Self(12);

    pub fn from_name(name: &str) -> Option<Self> {
        SEMANTIC_ROLES
            .iter()
            .find(|spec| spec.name == name)
            .map(|spec| Self(spec.id))
    }

    pub fn name(self) -> Option<&'static str> {
        SEMANTIC_ROLES
            .iter()
            .find(|spec| spec.id == self.0)
            .map(|spec| spec.name)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Trait(pub u32);

impl Trait {
    pub const DECLARATION: Self = Self(1);
    pub const SCOPE: Self = Self(2);

    pub fn from_name(name: &str) -> Option<Self> {
        SEMANTIC_TRAITS
            .iter()
            .find(|spec| spec.name == name)
            .map(|spec| Self(spec.id))
    }

    pub fn name(self) -> Option<&'static str> {
        SEMANTIC_TRAITS
            .iter()
            .find(|spec| spec.id == self.0)
            .map(|spec| spec.name)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TraitSet {
    bits: u32,
}

impl TraitSet {
    pub fn empty() -> Self {
        Self { bits: 0 }
    }

    pub fn insert(&mut self, value: Trait) {
        self.bits |= 1 << value.0;
    }

    pub fn contains(self, value: Trait) -> bool {
        self.bits & (1 << value.0) != 0
    }

    pub fn iter(self) -> impl Iterator<Item = Trait> {
        SEMANTIC_TRAITS
            .iter()
            .filter(move |spec| self.bits & (1 << spec.id) != 0)
            .map(|spec| Trait(spec.id))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticRoleSpec {
    pub name: &'static str,
    pub id: u32,
    pub required: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticTraitSpec {
    pub name: &'static str,
    pub id: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticKindSpec {
    pub name: &'static str,
    pub id: u32,
    pub roles: &'static [SemanticRoleSpec],
    pub traits: &'static [SemanticTraitSpec],
}

const SEMANTIC_KIND_MODULE_ROLES: &[SemanticRoleSpec] = &[SemanticRoleSpec {
    name: "body",
    id: 2,
    required: true,
}];
const SEMANTIC_KIND_MODULE_TRAITS: &[SemanticTraitSpec] = &[SemanticTraitSpec {
    name: "scope",
    id: 2,
}];
const SEMANTIC_KIND_DECLARATION_ROLES: &[SemanticRoleSpec] = &[SemanticRoleSpec {
    name: "name",
    id: 1,
    required: true,
}];
const SEMANTIC_KIND_DECLARATION_TRAITS: &[SemanticTraitSpec] = &[SemanticTraitSpec {
    name: "declaration",
    id: 1,
}];
const SEMANTIC_KIND_FUNCTION_ROLES: &[SemanticRoleSpec] = &[
    SemanticRoleSpec {
        name: "name",
        id: 1,
        required: false,
    },
    SemanticRoleSpec {
        name: "parameters",
        id: 3,
        required: true,
    },
    SemanticRoleSpec {
        name: "body",
        id: 2,
        required: true,
    },
];
const SEMANTIC_KIND_FUNCTION_TRAITS: &[SemanticTraitSpec] = &[
    SemanticTraitSpec {
        name: "declaration",
        id: 1,
    },
    SemanticTraitSpec {
        name: "scope",
        id: 2,
    },
];
const SEMANTIC_KIND_PARAMETER_ROLES: &[SemanticRoleSpec] = &[SemanticRoleSpec {
    name: "name",
    id: 1,
    required: true,
}];
const SEMANTIC_KIND_PARAMETER_TRAITS: &[SemanticTraitSpec] = &[SemanticTraitSpec {
    name: "declaration",
    id: 1,
}];
const SEMANTIC_KIND_CALL_ROLES: &[SemanticRoleSpec] = &[
    SemanticRoleSpec {
        name: "callee",
        id: 6,
        required: true,
    },
    SemanticRoleSpec {
        name: "arguments",
        id: 5,
        required: true,
    },
];
const SEMANTIC_KIND_MEMBER_ROLES: &[SemanticRoleSpec] = &[
    SemanticRoleSpec {
        name: "receiver",
        id: 4,
        required: true,
    },
    SemanticRoleSpec {
        name: "name",
        id: 1,
        required: true,
    },
];
const SEMANTIC_KIND_IDENTIFIER_ROLES: &[SemanticRoleSpec] = &[SemanticRoleSpec {
    name: "name",
    id: 1,
    required: false,
}];
const SEMANTIC_KIND_BLOCK_ROLES: &[SemanticRoleSpec] = &[SemanticRoleSpec {
    name: "body",
    id: 2,
    required: false,
}];
const SEMANTIC_KIND_BLOCK_TRAITS: &[SemanticTraitSpec] = &[SemanticTraitSpec {
    name: "scope",
    id: 2,
}];
const SEMANTIC_KIND_ASSIGNMENT_ROLES: &[SemanticRoleSpec] = &[
    SemanticRoleSpec {
        name: "left",
        id: 7,
        required: true,
    },
    SemanticRoleSpec {
        name: "right",
        id: 8,
        required: true,
    },
];
const SEMANTIC_KIND_BRANCH_ROLES: &[SemanticRoleSpec] = &[
    SemanticRoleSpec {
        name: "condition",
        id: 9,
        required: true,
    },
    SemanticRoleSpec {
        name: "body",
        id: 2,
        required: true,
    },
];
const SEMANTIC_KIND_LOOP_ROLES: &[SemanticRoleSpec] = &[
    SemanticRoleSpec {
        name: "condition",
        id: 9,
        required: false,
    },
    SemanticRoleSpec {
        name: "body",
        id: 2,
        required: true,
    },
];
const SEMANTIC_KIND_RETURN_ROLES: &[SemanticRoleSpec] = &[SemanticRoleSpec {
    name: "value",
    id: 11,
    required: false,
}];
const SEMANTIC_KIND_IMPORT_ROLES: &[SemanticRoleSpec] = &[SemanticRoleSpec {
    name: "source",
    id: 12,
    required: true,
}];
const SEMANTIC_KIND_IMPORT_TRAITS: &[SemanticTraitSpec] = &[SemanticTraitSpec {
    name: "declaration",
    id: 1,
}];
const SEMANTIC_KIND_FIELD_ROLES: &[SemanticRoleSpec] = &[
    SemanticRoleSpec {
        name: "name",
        id: 1,
        required: false,
    },
    SemanticRoleSpec {
        name: "value",
        id: 11,
        required: true,
    },
];
const SEMANTIC_KIND_OPERATOR_ROLES: &[SemanticRoleSpec] = &[
    SemanticRoleSpec {
        name: "left",
        id: 7,
        required: false,
    },
    SemanticRoleSpec {
        name: "right",
        id: 8,
        required: false,
    },
];
const SEMANTIC_KIND_CLASS_ROLES: &[SemanticRoleSpec] = &[
    SemanticRoleSpec {
        name: "name",
        id: 1,
        required: true,
    },
    SemanticRoleSpec {
        name: "body",
        id: 2,
        required: true,
    },
];
const SEMANTIC_KIND_CLASS_TRAITS: &[SemanticTraitSpec] = &[
    SemanticTraitSpec {
        name: "declaration",
        id: 1,
    },
    SemanticTraitSpec {
        name: "scope",
        id: 2,
    },
];
const SEMANTIC_KIND_NAMESPACE_ROLES: &[SemanticRoleSpec] = &[
    SemanticRoleSpec {
        name: "name",
        id: 1,
        required: false,
    },
    SemanticRoleSpec {
        name: "body",
        id: 2,
        required: true,
    },
];
const SEMANTIC_KIND_NAMESPACE_TRAITS: &[SemanticTraitSpec] = &[SemanticTraitSpec {
    name: "scope",
    id: 2,
}];

pub const SEMANTIC_ROLES: &[SemanticRoleSpec] = &[
    SemanticRoleSpec {
        name: "name",
        id: 1,
        required: false,
    },
    SemanticRoleSpec {
        name: "body",
        id: 2,
        required: false,
    },
    SemanticRoleSpec {
        name: "parameters",
        id: 3,
        required: false,
    },
    SemanticRoleSpec {
        name: "receiver",
        id: 4,
        required: false,
    },
    SemanticRoleSpec {
        name: "arguments",
        id: 5,
        required: false,
    },
    SemanticRoleSpec {
        name: "callee",
        id: 6,
        required: false,
    },
    SemanticRoleSpec {
        name: "left",
        id: 7,
        required: false,
    },
    SemanticRoleSpec {
        name: "right",
        id: 8,
        required: false,
    },
    SemanticRoleSpec {
        name: "condition",
        id: 9,
        required: false,
    },
    SemanticRoleSpec {
        name: "return_type",
        id: 10,
        required: false,
    },
    SemanticRoleSpec {
        name: "value",
        id: 11,
        required: false,
    },
    SemanticRoleSpec {
        name: "source",
        id: 12,
        required: false,
    },
];

pub const SEMANTIC_TRAITS: &[SemanticTraitSpec] = &[
    SemanticTraitSpec {
        name: "declaration",
        id: 1,
    },
    SemanticTraitSpec {
        name: "scope",
        id: 2,
    },
];

pub const SEMANTIC_KINDS: &[SemanticKindSpec] = &[
    SemanticKindSpec {
        name: "module",
        id: 1,
        roles: SEMANTIC_KIND_MODULE_ROLES,
        traits: SEMANTIC_KIND_MODULE_TRAITS,
    },
    SemanticKindSpec {
        name: "declaration",
        id: 2,
        roles: SEMANTIC_KIND_DECLARATION_ROLES,
        traits: SEMANTIC_KIND_DECLARATION_TRAITS,
    },
    SemanticKindSpec {
        name: "function",
        id: 3,
        roles: SEMANTIC_KIND_FUNCTION_ROLES,
        traits: SEMANTIC_KIND_FUNCTION_TRAITS,
    },
    SemanticKindSpec {
        name: "parameter",
        id: 4,
        roles: SEMANTIC_KIND_PARAMETER_ROLES,
        traits: SEMANTIC_KIND_PARAMETER_TRAITS,
    },
    SemanticKindSpec {
        name: "call",
        id: 5,
        roles: SEMANTIC_KIND_CALL_ROLES,
        traits: &[],
    },
    SemanticKindSpec {
        name: "member",
        id: 6,
        roles: SEMANTIC_KIND_MEMBER_ROLES,
        traits: &[],
    },
    SemanticKindSpec {
        name: "identifier",
        id: 7,
        roles: SEMANTIC_KIND_IDENTIFIER_ROLES,
        traits: &[],
    },
    SemanticKindSpec {
        name: "literal",
        id: 8,
        roles: &[],
        traits: &[],
    },
    SemanticKindSpec {
        name: "type",
        id: 9,
        roles: &[],
        traits: &[],
    },
    SemanticKindSpec {
        name: "block",
        id: 10,
        roles: SEMANTIC_KIND_BLOCK_ROLES,
        traits: SEMANTIC_KIND_BLOCK_TRAITS,
    },
    SemanticKindSpec {
        name: "assignment",
        id: 11,
        roles: SEMANTIC_KIND_ASSIGNMENT_ROLES,
        traits: &[],
    },
    SemanticKindSpec {
        name: "branch",
        id: 12,
        roles: SEMANTIC_KIND_BRANCH_ROLES,
        traits: &[],
    },
    SemanticKindSpec {
        name: "loop",
        id: 13,
        roles: SEMANTIC_KIND_LOOP_ROLES,
        traits: &[],
    },
    SemanticKindSpec {
        name: "return",
        id: 14,
        roles: SEMANTIC_KIND_RETURN_ROLES,
        traits: &[],
    },
    SemanticKindSpec {
        name: "import",
        id: 15,
        roles: SEMANTIC_KIND_IMPORT_ROLES,
        traits: SEMANTIC_KIND_IMPORT_TRAITS,
    },
    SemanticKindSpec {
        name: "table",
        id: 16,
        roles: &[],
        traits: &[],
    },
    SemanticKindSpec {
        name: "field",
        id: 17,
        roles: SEMANTIC_KIND_FIELD_ROLES,
        traits: &[],
    },
    SemanticKindSpec {
        name: "operator",
        id: 18,
        roles: SEMANTIC_KIND_OPERATOR_ROLES,
        traits: &[],
    },
    SemanticKindSpec {
        name: "comment",
        id: 19,
        roles: &[],
        traits: &[],
    },
    SemanticKindSpec {
        name: "class",
        id: 20,
        roles: SEMANTIC_KIND_CLASS_ROLES,
        traits: SEMANTIC_KIND_CLASS_TRAITS,
    },
    SemanticKindSpec {
        name: "namespace",
        id: 21,
        roles: SEMANTIC_KIND_NAMESPACE_ROLES,
        traits: SEMANTIC_KIND_NAMESPACE_TRAITS,
    },
    SemanticKindSpec {
        name: "string",
        id: 22,
        roles: &[],
        traits: &[],
    },
];

pub fn kind_by_id(id: u32) -> Option<&'static SemanticKindSpec> {
    SEMANTIC_KINDS.iter().find(|spec| spec.id == id)
}

pub fn kind_by_name(name: &str) -> Option<&'static SemanticKindSpec> {
    SEMANTIC_KINDS.iter().find(|spec| spec.name == name)
}

pub fn role_ids() -> BTreeMap<&'static str, u32> {
    SEMANTIC_ROLES
        .iter()
        .map(|spec| (spec.name, spec.id))
        .collect()
}

pub fn trait_ids() -> BTreeMap<&'static str, u32> {
    SEMANTIC_TRAITS
        .iter()
        .map(|spec| (spec.name, spec.id))
        .collect()
}
