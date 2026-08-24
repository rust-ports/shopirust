//! Port warnings when requested GraphiQL/localhost ports are unavailable.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortKind {
    Graphiql,
    Localhost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortDetail {
    pub kind: PortKind,
    pub requested: u16,
    pub actual: u16,
}

impl PortDetail {
    pub fn flag_to_remedy(&self) -> &'static str {
        match self.kind {
            PortKind::Graphiql => "--graphiql-port",
            PortKind::Localhost => "--localhost-port",
        }
    }

    pub fn label(&self) -> &'static str {
        match self.kind {
            PortKind::Graphiql => "GraphiQL",
            PortKind::Localhost => "localhost",
        }
    }
}

/// Returns human-readable warning strings (caller prints via CLI UI).
pub fn render_port_warnings(port_details: &[PortDetail]) -> Vec<String> {
    let warnings: Vec<&PortDetail> = port_details
        .iter()
        .filter(|w| w.requested != w.actual)
        .collect();
    if warnings.is_empty() {
        return vec![];
    }

    if warnings.len() == 1 {
        let w = warnings[0];
        return vec![format!(
            "A random port will be used for {} because {} is not available. Use the {} flag to choose a different port.",
            w.label(),
            w.requested,
            w.flag_to_remedy()
        )];
    }

    let kinds = warnings
        .iter()
        .map(|w| w.label())
        .collect::<Vec<_>>()
        .join(" and ");
    let flags = warnings
        .iter()
        .map(|w| w.flag_to_remedy())
        .collect::<Vec<_>>()
        .join(" and ");
    vec![format!(
        "Random ports will be used for {kinds} because the requested ports are not available. Use the {flags} flags."
    )]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_warnings_when_ports_match() {
        let details = vec![PortDetail {
            kind: PortKind::Localhost,
            requested: 3000,
            actual: 3000,
        }];
        assert!(render_port_warnings(&details).is_empty());
    }

    #[test]
    fn single_port_mismatch() {
        let details = vec![PortDetail {
            kind: PortKind::Graphiql,
            requested: 3457,
            actual: 4000,
        }];
        let msgs = render_port_warnings(&details);
        assert_eq!(msgs.len(), 1);
        assert!(msgs[0].contains("GraphiQL"));
        assert!(msgs[0].contains("--graphiql-port"));
    }

    #[test]
    fn multiple_port_mismatches() {
        let details = vec![
            PortDetail {
                kind: PortKind::Graphiql,
                requested: 1,
                actual: 2,
            },
            PortDetail {
                kind: PortKind::Localhost,
                requested: 3,
                actual: 4,
            },
        ];
        let msgs = render_port_warnings(&details);
        assert_eq!(msgs.len(), 1);
        assert!(msgs[0].contains("GraphiQL"));
        assert!(msgs[0].contains("localhost"));
    }
}
