/// Path-namespace color assignment for tile borders.
///
/// Sessions sharing a working directory receive matching colored borders.
/// Groups with only one session get a neutral default border.

use ratatui::style::Color;
use std::collections::HashMap;
use std::path::PathBuf;

const PALETTE: &[Color] = &[
    Color::Cyan,
    Color::Green,
    Color::Yellow,
    Color::Magenta,
    Color::Blue,
    Color::Red,
    Color::LightCyan,
    Color::LightGreen,
    Color::LightYellow,
    Color::LightMagenta,
    Color::LightBlue,
    Color::LightRed,
];

pub const DEFAULT_BORDER_COLOR: Color = Color::DarkGray;

/// Assign border colors based on working directory grouping.
///
/// Sessions that share a cwd get matching colors from a fixed palette.
/// Sessions with a unique cwd get `DEFAULT_BORDER_COLOR`.
/// Color assignment is stable: determined by sorted path order.
pub fn assign_border_colors(cwds: &[PathBuf]) -> HashMap<PathBuf, Color> {
    let mut path_counts: HashMap<PathBuf, usize> = HashMap::new();
    for cwd in cwds {
        *path_counts.entry(cwd.clone()).or_insert(0) += 1;
    }

    let mut grouped_paths: Vec<&PathBuf> = path_counts
        .iter()
        .filter(|(_, count)| **count > 1)
        .map(|(path, _)| path)
        .collect();
    grouped_paths.sort();

    let mut colors = HashMap::new();
    for (idx, path) in grouped_paths.iter().enumerate() {
        colors.insert((*path).clone(), PALETTE[idx % PALETTE.len()]);
    }

    for (path, count) in &path_counts {
        if *count <= 1 {
            colors.insert(path.clone(), DEFAULT_BORDER_COLOR);
        }
    }

    colors
}

/// Look up the border color for a given path.
pub fn border_color_for(colors: &HashMap<PathBuf, Color>, cwd: &PathBuf) -> Color {
    colors.get(cwd).copied().unwrap_or(DEFAULT_BORDER_COLOR)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_sessions_get_default_color() {
        let cwds = vec![
            PathBuf::from("/a"),
            PathBuf::from("/b"),
            PathBuf::from("/c"),
        ];
        let colors = assign_border_colors(&cwds);
        assert_eq!(colors[&PathBuf::from("/a")], DEFAULT_BORDER_COLOR);
        assert_eq!(colors[&PathBuf::from("/b")], DEFAULT_BORDER_COLOR);
    }

    #[test]
    fn grouped_sessions_get_matching_color() {
        let cwds = vec![
            PathBuf::from("/project"),
            PathBuf::from("/project"),
            PathBuf::from("/other"),
        ];
        let colors = assign_border_colors(&cwds);
        let project_color = colors[&PathBuf::from("/project")];
        assert_ne!(project_color, DEFAULT_BORDER_COLOR);
        assert_eq!(colors[&PathBuf::from("/other")], DEFAULT_BORDER_COLOR);
    }

    #[test]
    fn multiple_groups_get_different_colors() {
        let cwds = vec![
            PathBuf::from("/a"),
            PathBuf::from("/a"),
            PathBuf::from("/b"),
            PathBuf::from("/b"),
        ];
        let colors = assign_border_colors(&cwds);
        assert_ne!(colors[&PathBuf::from("/a")], colors[&PathBuf::from("/b")]);
    }

    #[test]
    fn empty_input() {
        let colors = assign_border_colors(&[]);
        assert!(colors.is_empty());
    }
}
