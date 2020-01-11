pub const APP_NAME: &str = env!("CARGO_PKG_NAME");
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub const MASTER_NOTE_TEMPLATE: &'static str = r"\documentclass[class=memoir, crop=false, oneside, 12pt]{standalone}

% document metadata
\author{ {{~name~}} }
\title{ {{~_subject.name~}} }
\date{ {{~_date~}} }

\begin{document}
% Frontmatter of the class note

{{#each _master.notes}}
\include{ {{~this._slug}}.tex}
{{/each }}

\end{document}
";

pub const NOTE_TEMPLATE: &'static str = r"\documentclass[class=memoir, crop=false, oneside, 14pt]{standalone}

% document metadata
\author{ {{~name~}} }
\title{ {{~_note.title~}} }
\date{ {{~_date~}} }

\begin{document}
Sample content.

{{_subject.name}}
\end{document}
";
