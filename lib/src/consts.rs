pub const APP_NAME: &str = env!("CARGO_PKG_NAME");
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub const MASTER_NOTE_TEMPLATE: &'static str = r"\documentclass[class=memoir, crop=false, oneside, 12pt]{standalone}

% document metadata
\author{ {{~profile.name~}} }
\title{ {{~subject.name~}} }
\date{ {{~reldate~}} }

\begin{document}
% Frontmatter of the class note

{{#each master.notes}}
Note: {{this.title}}
{{/each }}

\end{document}
";

pub const NOTE_TEMPLATE: &'static str = r"\documentclass[class=memoir, crop=false, oneside, 14pt]{standalone}

% document metadata
\author{ {{~profile.name~}} }
\title{ {{~note.title~}} }
\date{ {{~reldate~}} }

\begin{document}
Sample content.

{{subject.name}}
\end{document}
";
