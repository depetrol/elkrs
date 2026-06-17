//! Utilities from `org.eclipse.elk.core.util`.

use crate::graph::properties::{JavaString, PropertyHolder, PropertyMap};

/// A property holder storing spacing values
/// that apply to one element only, overriding the parent's spacings.
#[derive(Default, Clone, Debug, PartialEq)]
pub struct IndividualSpacings {
    pub properties: PropertyMap,
}

impl PropertyHolder for IndividualSpacings {
    fn properties(&self) -> &PropertyMap {
        &self.properties
    }
    fn properties_mut(&mut self) -> &mut PropertyMap {
        &mut self.properties
    }
}

impl JavaString for IndividualSpacings {
    fn java_string(&self) -> String {
        // The exact form never matters for layout. Mirror the option entries
        // instead for debuggability.
        let entries: Vec<String> =
            self.properties.entries().iter().map(|(k, v)| format!("{k}={}", v.to_java_string())).collect();
        format!("IndividualSpacings({})", entries.join(", "))
    }
}

