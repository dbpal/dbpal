use typescript_type_def::TypeDef;
use typescript_type_def::type_expr::{
    DefinedTypeInfo, Ident, NativeTypeInfo, ObjectField, TypeDefinition, TypeExpr, TypeInfo,
    TypeName, TypeObject, TypeString, TypeUnion,
};

use crate::db::HumanSize;
use crate::language_server::CompletionItem;
use crate::meta_value::MetaValue;

impl TypeDef for CompletionItem {
    const INFO: TypeInfo = TypeInfo::Defined(DefinedTypeInfo {
        def: TypeDefinition {
            docs: None,
            path: &[],
            name: Ident("CompletionItem"),
            generic_vars: &[],
            def: TypeExpr::Object(TypeObject {
                docs: None,
                index_signature: None,
                fields: &[
                    ObjectField {
                        docs: None,
                        name: TypeString {
                            docs: None,
                            value: "item",
                        },
                        optional: false,
                        r#type: TypeExpr::Name(TypeName::ident(Ident("string"))),
                    },
                    ObjectField {
                        docs: None,
                        name: TypeString {
                            docs: None,
                            value: "kind",
                        },
                        optional: false,
                        r#type: TypeExpr::Name(TypeName::ident(Ident("CompletionKind"))),
                    },
                    ObjectField {
                        docs: None,
                        name: TypeString {
                            docs: None,
                            value: "desc",
                        },
                        optional: false,
                        r#type: TypeExpr::Union(TypeUnion {
                            docs: None,
                            members: &[
                                TypeExpr::Name(TypeName::ident(Ident("string"))),
                                TypeExpr::Name(TypeName::ident(Ident("null"))),
                            ],
                        }),
                    },
                ],
            }),
        },
        generic_args: &[],
    });
}

impl TypeDef for MetaValue {
    const INFO: TypeInfo = TypeInfo::Defined(DefinedTypeInfo {
        def: TypeDefinition {
            docs: None,
            path: &[],
            name: Ident("MetaValue"),
            generic_vars: &[],
            def: TypeExpr::Object(TypeObject {
                docs: None,
                index_signature: None,
                fields: &[
                    ObjectField {
                        docs: None,
                        name: TypeString {
                            docs: None,
                            value: "name",
                        },
                        optional: false,
                        r#type: TypeExpr::Name(TypeName::ident(Ident("string"))),
                    },
                    ObjectField {
                        docs: None,
                        name: TypeString {
                            docs: None,
                            value: "value",
                        },
                        optional: false,
                        r#type: TypeExpr::Union(TypeUnion {
                            docs: None,
                            members: &[
                                TypeExpr::Name(TypeName::ident(Ident("string"))),
                                TypeExpr::Name(TypeName::ident(Ident("null"))),
                            ],
                        }),
                    },
                    ObjectField {
                        docs: None,
                        name: TypeString {
                            docs: None,
                            value: "desc",
                        },
                        optional: false,
                        r#type: TypeExpr::Union(TypeUnion {
                            docs: None,
                            members: &[
                                TypeExpr::Name(TypeName::ident(Ident("string"))),
                                TypeExpr::Name(TypeName::ident(Ident("null"))),
                            ],
                        }),
                    },
                ],
            }),
        },
        generic_args: &[],
    });
}

impl TypeDef for HumanSize {
    const INFO: TypeInfo = TypeInfo::Native(NativeTypeInfo {
        r#ref: TypeExpr::Name(TypeName {
            path: &[],
            name: Ident("string"),
            generic_args: &[],
        }),
    });
}
