use darling::FromMeta;
use syn::Expr;

/// Custom wrapper type for parsing expressions from attributes.
#[derive(Debug, Clone)]
pub struct ExprWrapper(pub(crate) Expr);

impl ExprWrapper {
    pub(crate) fn from_value(value: &syn::Lit) -> darling::Result<Self> {
        match value {
            syn::Lit::Str(s) => Self::from_string(&s.value()),
            _ => Err(darling::Error::custom("Expected string literal")),
        }
    }

    fn from_string(value: &str) -> darling::Result<Self> {
        syn::parse_str::<Expr>(value)
            .map(ExprWrapper)
            .map_err(|e| darling::Error::custom(format!("Failed to parse expression: {}", e)))
    }
}

impl FromMeta for ExprWrapper {
    fn from_value(value: &syn::Lit) -> darling::Result<Self> {
        ExprWrapper::from_value(value)
    }

    fn from_string(value: &str) -> darling::Result<Self> {
        ExprWrapper::from_string(value)
    }
}
