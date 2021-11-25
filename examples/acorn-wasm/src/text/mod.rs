//! WebAssembly 文本格式（WAT / WIT）的文档模型与写出。

pub mod wat;
pub mod wit;

pub use wat::{WatDocument, WatError, format_wat_document};
pub use wit::{WitError, WitInterface, WitPackage, format_wit_package};
