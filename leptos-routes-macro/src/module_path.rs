#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModulePath {
    idents: Vec<syn::Ident>,
}

impl ModulePath {
    pub fn root(root: syn::Ident) -> Self {
        Self {
            idents: vec![root]
        }
    }

    pub fn push(&mut self, ident: syn::Ident) {
        self.idents.push(ident);
    }

    pub fn without_first(&self) -> &[syn::Ident] {
        &self.idents[1..self.idents.len() - 1]
    }
}
