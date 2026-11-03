/// The capability layers (docs/rules/semantics.md). A rule that needs a layer the provider
/// does not answer, or the analysis depth does not allow, is skipped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    /// Declarations and what names bind to (and unused imports).
    Binding,
    /// Types of declarations and expressions (is a method async, does it return a task).
    Types,
    /// Constants and nullable flow state.
    Values,
    /// Control-flow graphs per body.
    Flow,
    /// References across the files of the project.
    Project,
}

/// A set of layers: what a provider answers, or what a file's rules need.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Layers {
    pub binding: bool,
    pub types: bool,
    pub values: bool,
    pub flow: bool,
    pub project: bool,
}

impl Layers {
    pub const NONE: Layers = Layers {
        binding: false,
        types: false,
        values: false,
        flow: false,
        project: false,
    };

    /// Parses the layer names providers advertise (`binding`, `types`, `values`, `flow`,
    /// `project`); unknown names are ignored.
    pub fn from_names<S: AsRef<str>>(names: &[S]) -> Self {
        let has = |n: &str| names.iter().any(|x| x.as_ref() == n);
        Layers {
            binding: has("binding"),
            types: has("types"),
            values: has("values"),
            flow: has("flow"),
            project: has("project"),
        }
    }

    pub fn insert(&mut self, layer: Layer) {
        match layer {
            Layer::Binding => self.binding = true,
            Layer::Types => self.types = true,
            Layer::Values => self.values = true,
            Layer::Flow => self.flow = true,
            Layer::Project => self.project = true,
        }
    }

    pub fn contains(&self, layer: Layer) -> bool {
        match layer {
            Layer::Binding => self.binding,
            Layer::Types => self.types,
            Layer::Values => self.values,
            Layer::Flow => self.flow,
            Layer::Project => self.project,
        }
    }

    pub fn is_empty(&self) -> bool {
        *self == Layers::default()
    }
}
