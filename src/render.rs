use crate::dfa::Dfa;
use std::{collections::BTreeMap, fmt::Write};

fn xml(value: &str) -> String {
    value
        .chars()
        .map(|c| match c {
            '&' => "&amp;".into(),
            '<' => "&lt;".into(),
            '>' => "&gt;".into(),
            '"' => "&quot;".into(),
            '\'' => "&apos;".into(),
            c if (c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
                || matches!(c, '\u{fffe}' | '\u{ffff}') =>
            {
                "\u{fffd}".into()
            }
            c => c.to_string(),
        })
        .collect()
}

fn abbreviated(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        value.to_owned()
    } else {
        value.chars().take(limit - 1).chain(['…']).collect()
    }
}

fn edges(dfa: &Dfa) -> BTreeMap<(usize, usize), Vec<&str>> {
    let mut edges: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for (q, row) in dfa.transitions.iter().enumerate() {
        for (a, &target) in row.iter().enumerate() {
            edges
                .entry((q, target))
                .or_default()
                .push(dfa.alphabet[a].as_str());
        }
    }
    edges
}

/// Portable SVG with grouped edges, start arrow, accepting double circles and member tooltips.
/// Circular layout targets teaching examples; DOT is also available for external layout engines.
pub fn svg(dfa: &Dfa, title: &str, members: Option<&[Vec<String>]>) -> String {
    let n = dfa.names.len();
    let radius = if n == 1 {
        0.0
    } else {
        (n as f64 * 29.0).max(145.0)
    };
    let width = (radius * 2.0 + 260.0).max(760.0);
    let height = (radius * 2.0 + 290.0).max(480.0);
    let center = (width / 2.0, height / 2.0 + 20.0);
    let points: Vec<_> = (0..n)
        .map(|q| {
            let angle = std::f64::consts::TAU * q as f64 / n as f64 - std::f64::consts::FRAC_PI_2;
            (
                center.0 + radius * angle.cos(),
                center.1 + radius * angle.sin(),
            )
        })
        .collect();
    let mut out = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width:.0}" height="{height:.0}" viewBox="0 0 {width:.0} {height:.0}" role="img" aria-labelledby="title desc">
<title id="title">{}</title><desc id="desc">Complete deterministic automaton. Double circles mark accepting states. Hover over states to see full names and merged members.</desc>
<defs><marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="#64748b"/></marker></defs>
<rect width="100%" height="100%" rx="18" fill="#f8fafc"/>
<g font-family="system-ui, sans-serif"><text x="32" y="42" font-size="24" font-weight="700" fill="#0f172a">{}</text>
<text x="32" y="69" font-size="13" fill="#475569">{} states · {} symbols · double circle = accepting</text>
"##,
        xml(title),
        xml(title),
        n,
        dfa.alphabet.len()
    );
    for ((q, target), symbols) in edges(dfa) {
        let (x, y) = points[q];
        let (tx, ty) = points[target];
        let (path, lx, ly) = if q == target {
            (
                format!(
                    "M {:.2} {:.2} C {:.2} {:.2}, {:.2} {:.2}, {:.2} {:.2}",
                    x - 20.0,
                    y - 24.0,
                    x - 65.0,
                    y - 99.0,
                    x + 65.0,
                    y - 99.0,
                    x + 20.0,
                    y - 24.0
                ),
                x,
                y - 83.0,
            )
        } else {
            let length = (tx - x).hypot(ty - y);
            let (ux, uy) = ((tx - x) / length, (ty - y) / length);
            let (cx, cy) = ((x + tx) / 2.0 - uy * 32.0, (y + ty) / 2.0 + ux * 32.0);
            let sl = (cx - x).hypot(cy - y);
            let tl = (tx - cx).hypot(ty - cy);
            let (sx, sy) = (x + 34.0 * (cx - x) / sl, y + 34.0 * (cy - y) / sl);
            let (ex, ey) = (tx - 36.0 * (tx - cx) / tl, ty - 36.0 * (ty - cy) / tl);
            (
                format!("M {sx:.2} {sy:.2} Q {cx:.2} {cy:.2} {ex:.2} {ey:.2}"),
                (sx + 2.0 * cx + ex) / 4.0,
                (sy + 2.0 * cy + ey) / 4.0,
            )
        };
        let label = symbols.join(", ");
        let label_width = label.chars().count() as f64 * 8.0 + 12.0;
        writeln!(out, r##"<path d="{path}" fill="none" stroke="#64748b" stroke-width="1.6" marker-end="url(#arrow)"/><rect x="{:.2}" y="{:.2}" width="{label_width:.2}" height="20" rx="5" fill="#f8fafc"/><text x="{lx:.2}" y="{:.2}" text-anchor="middle" font-size="13" fill="#334155">{}</text>"##, lx - label_width / 2.0, ly - 12.0, ly + 3.0, xml(&label)).unwrap();
    }
    let (sx, sy) = points[dfa.start];
    writeln!(out, r##"<path d="M {:.2} {sy:.2} L {:.2} {sy:.2}" stroke="#64748b" stroke-width="2" marker-end="url(#arrow)"/><text x="{:.2}" y="{:.2}" text-anchor="middle" font-size="11" fill="#475569">start</text>"##, sx - 90.0, sx - 37.0, sx - 70.0, sy - 10.0).unwrap();
    for (q, &(x, y)) in points.iter().enumerate() {
        let detail = members
            .map(|groups| format!("{} = {{{}}}", dfa.names[q], groups[q].join(", ")))
            .unwrap_or_else(|| dfa.names[q].clone());
        let (fill, stroke) = if dfa.accepting[q] {
            ("#fef3c7", "#92400e")
        } else {
            ("#ccfbf1", "#115e59")
        };
        writeln!(out, r##"<g><title>{}</title><circle cx="{x:.2}" cy="{y:.2}" r="32" fill="{fill}" stroke="{stroke}" stroke-width="2"/>"##, xml(&detail)).unwrap();
        if dfa.accepting[q] {
            writeln!(out, r##"<circle cx="{x:.2}" cy="{y:.2}" r="27" fill="none" stroke="{stroke}" stroke-width="1.5"/>"##).unwrap();
        }
        writeln!(out, r##"<text x="{x:.2}" y="{:.2}" text-anchor="middle" font-size="12" font-weight="600" fill="#0f172a">{}</text>"##, y + 4.0, xml(&abbreviated(&dfa.names[q], 7))).unwrap();
        if let Some(members) = members {
            let label = abbreviated(&format!("{{{}}}", members[q].join(", ")), 26);
            let label_width = label.chars().count() as f64 * 7.0 + 8.0;
            writeln!(out, r##"<rect x="{:.2}" y="{:.2}" width="{label_width:.2}" height="18" rx="4" fill="#f8fafc"/><text x="{x:.2}" y="{:.2}" text-anchor="middle" font-size="11" fill="#475569">{}</text>"##, x - label_width / 2.0, y + 37.0, y + 50.0, xml(&label)).unwrap();
        }
        out.push_str("</g>\n");
    }
    out.push_str("</g></svg>\n");
    out
}

fn dot_quote(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
    )
}

pub fn dot(dfa: &Dfa, members: Option<&[Vec<String>]>) -> String {
    let mut out = String::from(
        "digraph DFA {\n  rankdir=LR;\n  node [shape=circle];\n  start [shape=point];\n",
    );
    writeln!(out, "  start -> q{};", dfa.start).unwrap();
    for (q, name) in dfa.names.iter().enumerate() {
        let label = members
            .map(|m| format!("{name}\n{{{}}}", m[q].join(", ")))
            .unwrap_or_else(|| name.clone());
        writeln!(
            out,
            "  q{q} [label={}, shape={}];",
            dot_quote(&label),
            if dfa.accepting[q] {
                "doublecircle"
            } else {
                "circle"
            }
        )
        .unwrap();
    }
    for ((q, target), symbols) in edges(dfa) {
        writeln!(
            out,
            "  q{q} -> q{target} [label={}];",
            dot_quote(&symbols.join(", "))
        )
        .unwrap();
    }
    out.push_str("}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_labels_and_groups_parallel_transitions() {
        let dfa = Dfa {
            alphabet: vec!["&".into(), "<".into()],
            names: vec!["<script>\"&".into()],
            start: 0,
            accepting: vec![true],
            transitions: vec![vec![0, 0]],
        };
        let svg = svg(&dfa, "Test <&>", None);
        assert!(!svg.contains("<script>"));
        assert!(svg.contains("&amp;, &lt;"));
        assert!(svg.contains("r=\"27\""));
        let dot = dot(&dfa, None);
        assert_eq!(dot.matches("q0 -> q0").count(), 1);
        assert!(dot.contains("\\\""));
    }
}
