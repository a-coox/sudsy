//! `rule!`: declares a Rust rule, its struct and its metadata, around a normal `impl Rule` body.

/// Declares a rule: the struct, its [`RuleMeta`](crate::RuleMeta) (with empty `requires`, `uses`
/// and `options` unless given) and the `meta`, `kinds` and `tags` methods of [`Rule`](crate::Rule).
/// The `check*`/`collect`/`finish` methods follow the metadata as ordinary functions.
///
/// ```ignore
/// rule! {
///     pub NoEmptyStatement {
///         id: "no-empty-statement",
///         category: Style,
///         default: Off,
///         fix: Safe,                       // omitted: no fix
///         description: "A statement that is only `;`.",
///         example: "if (a) ;   // remove it",
///         requires: [Types],               // optional: `Layer` names; so are `uses` and `options`
///         kinds: &[kind::EMPTY_STATEMENT], // optional: `check` is called for these kinds
///     }
///
///     fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) { /* ... */ }
/// }
/// ```
#[macro_export]
macro_rules! rule {
    (
        $vis:vis $name:ident {
            id: $id:expr,
            category: $category:ident,
            default: $level:ident,
            $(fix: $fix:ident,)?
            description: $description:expr,
            example: $example:expr,
            $(requires: [$($requires:ident),*],)?
            $(uses: [$($uses:ident),*],)?
            $(options: [$($options:expr),* $(,)?],)?
            $(kinds: $kinds:expr,)?
            $(tags: $tags:expr,)?
        }
        $($items:item)*
    ) => {
        $vis struct $name;

        impl $crate::Rule for $name {
            fn meta(&self) -> &'static $crate::RuleMeta {
                static META: $crate::RuleMeta = $crate::RuleMeta {
                    id: $id,
                    category: $crate::Category::$category,
                    description: $description,
                    example: $example,
                    default_level: $crate::rule::Level::$level,
                    fix: $crate::__fix!($($fix)?),
                    requires: &[$($($crate::Layer::$requires),*)?],
                    uses: &[$($($crate::Layer::$uses),*)?],
                    options: &[$($($options),*)?],
                };
                &META
            }

            $(fn kinds(&self) -> &[$crate::rule::SyntaxKind] {
                $kinds
            })?

            $(fn tags(&self) -> &[&'static str] {
                $tags
            })?

            $($items)*
        }
    };
}

/// The `fix:` of [`rule!`]: absent is no fix.
#[doc(hidden)]
#[macro_export]
macro_rules! __fix {
    () => {
        None
    };
    ($kind:ident) => {
        Some($crate::FixKind::$kind)
    };
}
