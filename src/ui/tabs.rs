//! The tab and focus enums: which pane owns navigation, and which section
//! of the workspace the content pane shows.

use crate::model::SectionKind;

/// The tabs of an RCA workspace, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    /// What broke ([`SectionKind::Summary`]).
    Summary,
    /// What happened when ([`SectionKind::Timeline`]).
    Timeline,
    /// Why it broke ([`SectionKind::RootCause`]).
    RootCause,
    /// Who/what was affected ([`SectionKind::Impact`]).
    Impact,
    /// How to fix it ([`SectionKind::Remediation`]).
    Remediation,
    /// ASCII diagrams from the workspace's `diagrams/` directory.
    Diagrams,
    /// Raw evidence and loose ends ([`SectionKind::Notes`]).
    Notes,
    /// The live investigation log ([`SectionKind::Log`]).
    Log,
}

impl Tab {
    /// Every tab, in display order.
    pub const ALL: [Self; 8] = [
        Self::Summary,
        Self::Timeline,
        Self::RootCause,
        Self::Impact,
        Self::Remediation,
        Self::Diagrams,
        Self::Notes,
        Self::Log,
    ];

    /// The tab title shown in the tab bar.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Diagrams => "Diagrams",
            Self::Summary => SectionKind::Summary.title(),
            Self::Timeline => SectionKind::Timeline.title(),
            Self::RootCause => SectionKind::RootCause.title(),
            Self::Impact => SectionKind::Impact.title(),
            Self::Remediation => SectionKind::Remediation.title(),
            Self::Notes => SectionKind::Notes.title(),
            Self::Log => SectionKind::Log.title(),
        }
    }

    /// The markdown section backing this tab; `None` for [`Tab::Diagrams`].
    #[must_use]
    pub fn section(self) -> Option<SectionKind> {
        match self {
            Self::Summary => Some(SectionKind::Summary),
            Self::Timeline => Some(SectionKind::Timeline),
            Self::RootCause => Some(SectionKind::RootCause),
            Self::Impact => Some(SectionKind::Impact),
            Self::Remediation => Some(SectionKind::Remediation),
            Self::Notes => Some(SectionKind::Notes),
            Self::Log => Some(SectionKind::Log),
            Self::Diagrams => None,
        }
    }

    /// Position of this tab within [`Tab::ALL`].
    #[must_use]
    pub fn index(self) -> usize {
        // `position` is infallible: every variant is in ALL by construction.
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }

    /// The next tab, wrapping.
    #[must_use]
    pub fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    /// The previous tab, wrapping.
    #[must_use]
    pub fn prev(self) -> Self {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

/// Which pane owns navigation keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// The workspace list on the left.
    List,
    /// The tabbed content pane on the right.
    Content,
}

#[cfg(test)]
#[path = "tests/tabs.rs"]
mod tests;
