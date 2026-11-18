//! WAT 文本文档模型与写出（仅文本格式，不承担解析职责）。

use std::fmt::{Display, Formatter};

/// `WAT` 文本错误（保留给未来文本前端适配层）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatError {
    /// 文本不是合法的 `module` 形式。
    InvalidModule(String),
    /// 括号没有正确闭合。
    UnbalancedParentheses,
    /// 字符串字面量没有结束。
    UnterminatedString,
}

impl Display for WatError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidModule(message) => write!(f, "invalid `WAT` module: {message}"),
            Self::UnbalancedParentheses => write!(f, "`WAT` module parentheses are unbalanced"),
            Self::UnterminatedString => write!(f, "`WAT` string literal is unterminated"),
        }
    }
}

impl std::error::Error for WatError {}

/// `WAT` 文本文档模型。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WatDocument {
    /// 可选模块名。
    pub module_name: Option<String>,
    /// 顶层模块字段文本。
    pub fields: Vec<String>,
}

impl WatDocument {
    /// 创建一个新的空 `WAT` 模块。
    pub fn new() -> Self {
        Self::default()
    }

    /// 追加一个顶层字段。
    pub fn push_field(&mut self, field: impl Into<String>) {
        self.fields.push(field.into());
    }
}

/// 将 WAT 文档写成文本。
pub fn format_wat_document(document: &WatDocument) -> String {
    let mut result = String::from("(module");
    if let Some(module_name) = &document.module_name {
        result.push(' ');
        result.push_str(module_name);
    }

    if document.fields.is_empty() {
        result.push(')');
        return result;
    }

    for field in &document.fields {
        result.push('\n');
        result.push_str("  ");
        result.push_str(field.trim());
    }
    result.push('\n');
    result.push(')');
    result
}
