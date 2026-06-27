//! Shared maud components for the table-based list views.
//!
//! These centralise the "identity cell" (bold name + source chip + muted
//! filename subline), the copyable hash cell, and the small clipboard handler
//! so every table renders them identically.

use maud::{Markup, PreEscaped, html};

use crate::db::mod_association::ModAssociation;

/// Middle-truncate a string to roughly `max` characters, keeping the head and
/// tail (which carry the most signal in mod filenames). Operates on `char`
/// boundaries so it is safe for non-ASCII input.
pub fn middle_truncate(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        return s.to_string();
    }
    // Reserve one char for the ellipsis; weight the head a little heavier.
    let budget = max.saturating_sub(1);
    let head = (budget * 2) / 3;
    let tail = budget - head;
    let head_s: String = chars[..head].iter().collect();
    let tail_s: String = chars[chars.len() - tail..].iter().collect();
    format!("{head_s}…{tail_s}")
}

/// Emit a `<colgroup>` assigning each column a width class. Used with
/// `table-layout: fixed` so the identity column (`col-mod`, left width-less)
/// absorbs the remaining space while the data columns stay compact — and no
/// row's content can ever blow the table past its container.
pub fn colgroup(cols: &[&str]) -> Markup {
    html! {
        colgroup {
            @for c in cols {
                col class=(c);
            }
        }
    }
}

/// The shared identity cell. `primary` is the bold first line; `secondary` (when
/// present) renders as a muted, middle-truncated monospace subline with the full
/// value in a `title` tooltip; `chip` is an optional `(label, css_class)` pill.
pub fn identity_cell(
    href: &str,
    primary: &str,
    secondary: Option<&str>,
    chip: Option<(&str, &str)>,
) -> Markup {
    html! {
        td.identity {
            a.identity-link href=(href) {
                span.identity-primary {
                    span.identity-name { (primary) }
                    @if let Some((label, class)) = chip {
                        span class=(format!("source-chip {class}")) { (label) }
                    }
                }
                @if let Some(sub) = secondary {
                    @if !sub.is_empty() {
                        span.identity-sub title=(sub) { (middle_truncate(sub, 52)) }
                    }
                }
            }
        }
    }
}

/// Identity cell for a mod row. Promotes the filename to the primary line when
/// no name is known, and tags the row with the source chip from `assoc`.
pub fn mod_identity_cell(
    href: &str,
    disk_filename: Option<&str>,
    assoc: Option<&ModAssociation>,
) -> Markup {
    let name = assoc
        .and_then(|a| a.name.as_deref())
        .filter(|s| !s.is_empty());
    let filename = disk_filename
        .or_else(|| assoc.map(|a| a.filename.as_str()))
        .filter(|s| !s.is_empty());

    let primary = name.or(filename).unwrap_or("Unknown");
    // Only show the filename subline when it is not already the primary line.
    let secondary = if name.is_some() { filename } else { None };
    let chip = assoc.map(|a| (a.source.source_chip_label(), a.source.source_chip_class()));

    identity_cell(href, primary, secondary, chip)
}

/// Identity cell for a modlist row: name on top, its own filename underneath.
/// Modlists have no download source, so there is no chip.
pub fn modlist_identity_cell(href: &str, name: &str, filename: &str) -> Markup {
    let secondary = Some(filename).filter(|s| !s.is_empty());
    identity_cell(href, name, secondary, None)
}

/// A hash cell rendering the full (short) hash on one line as a click-to-copy
/// button — handy for pasting into CLI flags like `prune --keep <hash>`.
pub fn hash_cell(hash: &str) -> Markup {
    html! {
        td.hash {
            button.hash-copy type="button" data-hash=(hash) title="Click to copy full hash" {
                code { (hash) }
            }
        }
    }
}

/// Tiny delegated click handler that copies `data-hash` to the clipboard and
/// flashes the button. Include once per page that renders [`hash_cell`].
pub fn clipboard_script() -> Markup {
    html! {
        script {
            (PreEscaped(r#"
document.addEventListener('click', function (e) {
  var btn = e.target.closest('.hash-copy');
  if (!btn) return;
  e.preventDefault();
  var text = btn.dataset.hash;
  try {
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(text);
    } else {
      var ta = document.createElement('textarea');
      ta.value = text;
      ta.style.position = 'fixed';
      ta.style.opacity = '0';
      document.body.appendChild(ta);
      ta.select();
      document.execCommand('copy');
      ta.remove();
    }
  } catch (_) {}
  btn.classList.add('copied');
  setTimeout(function () { btn.classList.remove('copied'); }, 1200);
});
"#))
        }
    }
}
