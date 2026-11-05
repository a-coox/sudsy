//! tree-sitter-xml node name -> `grammars/xml.grammar` kind. Anonymous tokens are named by their
//! text. References, CDATA sections, processing instructions, the XML declaration and the DOCTYPE
//! are tokens: what is inside them stays inside. `element` always wraps its tags, so an empty
//! element is an `Element` holding an `EmptyTag`.

use core_treesitter::Mapping;

pub const MAPPING: Mapping = Mapping {
    whitespace: "Whitespace",
    newline: "Newline",
    eof: "Eof",
    unknown: "Unknown",
    wrap: &[
        ("CharData", "Chunk"),
        ("Reference", "Chunk"),
        ("CData", "Chunk"),
        ("PI", "Chunk"),
        ("XmlDecl", "Chunk"),
        ("Doctype", "Chunk"),
    ],
    kinds: &[
        ("document", "Document"),
        ("element", "Element"),
        ("STag", "STag"),
        ("EmptyElemTag", "EmptyTag"),
        ("ETag", "ETag"),
        ("Attribute", "Attribute"),
        ("Comment", "Comment"),
        ("<", "Lt"),
        ("</", "LtSlash"),
        (">", "Gt"),
        ("/>", "SlashGt"),
        ("=", "Equals"),
        ("Name", "Name"),
        ("AttValue", "AttValue"),
        ("CharData", "CharData"),
        ("EntityRef", "Reference"),
        ("CharRef", "Reference"),
        ("CDSect", "CData"),
        ("PI", "PI"),
        ("StyleSheetPI", "PI"),
        ("XmlModelPI", "PI"),
        ("XMLDecl", "XmlDecl"),
        ("doctypedecl", "Doctype"),
    ],
};
