pub const APP_NAME: &str = env!("CARGO_PKG_NAME");
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub const MASTER_NOTE_TEMPLATE: &'static str = r"\documentclass[class=memoir, crop=false, oneside, 12pt]{standalone}

% document metadata
\author{ {{~profile.name~}} }
\title{ {{~subject.name~}} }
\date{ {{~date~}} }

\begin{document}
% Frontmatter of the class note

{{#each master.notes}}
\include{ {{~this._slug}}.tex}
{{/each }}

\end{document}
";

pub const NOTE_TEMPLATE: &'static str = r"\documentclass[class=memoir, crop=false, oneside, 14pt]{standalone}

% document metadata
\author{ {{~profile.name~}} }
\title{ {{~note.title~}} }
\date{ {{~date~}} }

\begin{document}
Sample content.

{{subject.name}}
\end{document}
";
