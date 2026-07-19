use colored::Colorize;
use crate::output::token::strip_ansi;

pub fn render_tabular_data(data: &[Vec<String>], first_column_subdued: bool) -> String {
    if data.is_empty() {
        return String::new();
    }

    let num_cols = data.iter().map(|r| r.len()).max().unwrap_or(0);
    if num_cols == 0 {
        return String::new();
    }

    let column_widths: Vec<usize> = (0..num_cols)
        .map(|col| {
            data.iter()
                .map(|row| {
                    row.get(col)
                        .map(|c| strip_ansi(c).len())
                        .unwrap_or(0)
                })
                .max()
                .unwrap_or(0)
        })
        .collect();

    let mut out = String::new();
    for row in data {
        for (col, cell) in row.iter().enumerate() {
            let display = if col == 0 && first_column_subdued {
                cell.dimmed().to_string()
            } else {
                cell.clone()
            };
            let padding = column_widths[col].saturating_sub(strip_ansi(cell).len());
            out.push_str(&display);
            if col < row.len() - 1 {
                out.push_str(&" ".repeat(padding + 2));
            }
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tabular_data_empty() {
        assert_eq!(render_tabular_data(&[], false), "");
    }

    #[test]
    fn test_tabular_data_basic() {
        let data = vec![
            vec!["Name".into(), "Role".into()],
            vec!["Alice".into(), "Admin".into()],
            vec!["Bob".into(), "Editor".into()],
        ];
        let result = render_tabular_data(&data, false);
        assert!(result.contains("Name"));
        assert!(result.contains("Alice"));
        assert!(result.contains("Bob"));
        assert!(result.contains("Admin"));
        assert!(result.contains("Editor"));
    }

    #[test]
    fn test_tabular_data_aligned_columns() {
        let data = vec![
            vec!["A".into(), "B".into()],
            vec!["LongName".into(), "C".into()],
        ];
        let result = render_tabular_data(&data, false);
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_tabular_data_first_column_subdued() {
        colored::control::set_override(true);
        let data = vec![vec!["key".into(), "value".into()]];
        let result = render_tabular_data(&data, true);
        assert!(result.contains("\x1b[2m"));
        assert!(result.contains("key"));
        assert!(result.contains("value"));
        colored::control::set_override(false);
    }

    #[test]
    fn test_tabular_data_uneven_rows() {
        let data = vec![
            vec!["a".into(), "b".into()],
            vec!["c".into()],
        ];
        let result = render_tabular_data(&data, false);
        assert!(result.contains("a"));
        assert!(result.contains("c"));
    }
}
