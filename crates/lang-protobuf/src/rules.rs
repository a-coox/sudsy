//! The Rust rules of Protobuf: the buf lint checks that compare names with each other or read the
//! file name. The checks on one node's own text are pattern files in `rules/`.

use crate::generated::kind as k;
use core_rules::rule::FileContext;
use core_rules::{Rule, RuleContext, rule};
use core_syntax::{SyntaxKind, SyntaxNode, SyntaxToken, TextRange, TextSize};

pub static RULES: &[&dyn Rule] = &[
    &EnumFirstValueZero,
    &EnumValuePrefix,
    &RpcRequestStandardName,
    &RpcResponseStandardName,
    &FileLowerSnakeCase,
];

fn child(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxNode> {
    node.children().find(|c| c.kind() == kind)
}

/// The own token of a node with this kind.
fn token(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == kind)
}

fn name_of(node: &SyntaxNode, name_kind: SyntaxKind) -> Option<String> {
    child(node, name_kind).map(|n| n.text().to_string().trim().to_string())
}

/// buf's `ToUpperSnakeCase`: a word starts at a capital after a lower case letter or digit, and at
/// the last capital of a run that a lower case letter follows (`HTTPRequest` is `HTTP_REQUEST`).
fn upper_snake(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::new();
    for (i, c) in chars.iter().enumerate() {
        if c.is_uppercase() && i > 0 {
            let before = chars[i - 1];
            let after = chars.get(i + 1);
            if before.is_lowercase()
                || before.is_ascii_digit()
                || (before.is_uppercase() && after.is_some_and(|a| a.is_lowercase()))
            {
                out.push('_');
            }
        }
        out.extend(c.to_uppercase());
    }
    out
}

rule! {
    pub EnumFirstValueZero {
        id: "enum-first-value-zero",
        category: Correctness,
        default: Off,
        description: "The first value of an enum is not 0 (buf lint ENUM_FIRST_VALUE_ZERO).",
        example: "enum E { A = 1; }   // reported at the 1",
        kinds: &[k::ENUM_BODY],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(first) = child(node, k::ENUM_FIELD) else { return };
        let Some(number) = token(&first, k::INT) else { return };
        let text = number.text();
        let zero = text.trim_start_matches("0x").trim_start_matches("0X").trim_start_matches('0').is_empty();
        if !zero {
            ctx.report(number.text_range(), "the first value of an enum must be 0");
        }
    }
}

rule! {
    pub EnumValuePrefix {
        id: "enum-value-prefix",
        category: Style,
        default: Off,
        description: "An enum value does not start with the enum's name in UPPER_SNAKE_CASE and an \
                  underscore (buf lint ENUM_VALUE_PREFIX).",
        example: "enum FooBar { BAZ = 0; }   // FOO_BAR_BAZ",
        kinds: &[k::ENUM],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let (Some(name), Some(body)) = (name_of(node, k::ENUM_NAME), child(node, k::ENUM_BODY)) else {
            return;
        };
        let prefix = format!("{}_", upper_snake(&name));
        for field in body.children().filter(|c| c.kind() == k::ENUM_FIELD) {
            if let Some(ident) = token(&field, k::IDENT).filter(|t| !t.text().starts_with(&prefix)) {
                ctx.report(
                    ident.text_range(),
                    format!("the value `{}` should start with `{prefix}`", ident.text()),
                );
            }
        }
    }
}

/// Reports the request (`index` 0) or response (1) type of an rpc whose name is neither
/// `<Rpc><suffix>` nor `<Service><Rpc><suffix>`.
fn check_rpc_type(node: &SyntaxNode, ctx: &mut RuleContext<'_>, index: usize, suffix: &str) {
    let Some(ty) = node
        .children()
        .filter(|c| c.kind() == k::MESSAGE_TYPE)
        .nth(index)
    else {
        return;
    };
    let Some(rpc) = name_of(node, k::RPC_NAME) else {
        return;
    };
    let service = node
        .parent()
        .and_then(|s| name_of(&s, k::SERVICE_NAME))
        .unwrap_or_default();
    let text = ty.text().to_string();
    let short = text
        .trim()
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_string();
    if short != format!("{rpc}{suffix}") && short != format!("{service}{rpc}{suffix}") {
        ctx.report(
            ty.text_range(),
            format!("the type should be named `{rpc}{suffix}` or `{service}{rpc}{suffix}`"),
        );
    }
}

rule! {
    pub RpcRequestStandardName {
        id: "rpc-request-standard-name",
        category: Style,
        default: Off,
        description: "The request type of an rpc is not named `<Rpc>Request` or \
                  `<Service><Rpc>Request` (buf lint RPC_REQUEST_STANDARD_NAME).",
        example: "service S { rpc Get(Foo) returns (GetResponse); }   // GetRequest",
        kinds: &[k::RPC],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        check_rpc_type(node, ctx, 0, "Request");
    }
}

rule! {
    pub RpcResponseStandardName {
        id: "rpc-response-standard-name",
        category: Style,
        default: Off,
        description: "The response type of an rpc is not named `<Rpc>Response` or \
                  `<Service><Rpc>Response` (buf lint RPC_RESPONSE_STANDARD_NAME).",
        example: "service S { rpc Get(GetRequest) returns (Foo); }   // GetResponse",
        kinds: &[k::RPC],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        check_rpc_type(node, ctx, 1, "Response");
    }
}

rule! {
    pub FileLowerSnakeCase {
        id: "file-lower-snake-case",
        category: Style,
        default: Off,
        description: "The file name (without `.proto`) is not lower_snake_case (buf lint \
                  FILE_LOWER_SNAKE_CASE).",
        example: "FooBar.proto",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let stem = file.path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
        let ok = stem.split('_').all(|w| {
            !w.is_empty() && w.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        });
        if !ok {
            let start = TextSize::from(0);
            ctx.report(TextRange::new(start, start), "the file name should be lower_snake_case");
        }
    }
}
