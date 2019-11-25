pub const TEXTURE_NOTES_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const TEXTURE_NOTES_DIR_NAME: &str = "texture-notes-profile";
pub const TEXTURE_NOTES_METADATA_FILENAME: &str = "profile.json";
pub const TEXTURE_NOTES_STYLES_DIR_NAME: &str = "styles";

pub const SQLITE_SCHEMA: &str = "PRAGMA foreign_key = ON;

CREATE TABLE IF NOT EXISTS subjects (
    id INTEGER,
    name TEXT UNIQUE NOT NULL,
    slug TEXT UNIQUE NOT NULL, 
    datetime_modified DATETIME NOT NULL,
    PRIMARY KEY(id),
    CHECK(
        TYPEOF(name) == 'text' AND
        LENGTH(name) <= 128 AND
        
        TYPEOF(datetime_modified) == 'text'
    )
);

CREATE TABLE IF NOT EXISTS notes (
    id INTEGER,
    title TEXT NOT NULL,
    slug TEXT NOT NULL, 
    subject_id INTEGER NOT NULL,
    datetime_modified DATETIME NOT NULL,
    PRIMARY KEY(id),
    FOREIGN KEY(subject_id) REFERENCES subjects(id)
        ON DELETE CASCADE
        ON UPDATE CASCADE,
    CHECK (
        -- checking if the title is a string with less than 512 characters
        TYPEOF(title) == 'text' AND
        LENGTH(title) <= 256 AND
        LOWER(title) NOT IN ('main', 'graphics') AND

        -- checking if the datetime is indeed in ISO format
        TYPEOF(datetime_modified) == 'text'
    )
);

CREATE TRIGGER IF NOT EXISTS unique_filename_note_check BEFORE INSERT ON notes 
BEGIN
    SELECT 
    CASE 
        WHEN (SELECT COUNT(slug) FROM notes WHERE subject_id == NEW.subject_id AND slug == NEW.slug) >= 1 
            THEN RAISE(FAIL, \"There's already a note with the filename under the specified subject.\")
        WHEN (SELECT COUNT(title) FROM notes WHERE subject_id == NEW.subject_id AND title == NEW.title) >= 1 
            THEN RAISE(FAIL, \"There's already a note with the same title under the specified subject.\")
    END;
END;

-- creating an index for the notes
CREATE INDEX IF NOT EXISTS notes_index ON notes(title, subject_id);
";

pub const MASTER_NOTE_TEMPLATE: &'static str = r"\documentclass[class=memoir, crop=false, oneside, 12pt]{{standalone}}

% document metadata
\author{{{author}}}
\title{{{title}}}
\date{{{date}}}

\begin{{document}}
% Frontmatter of the class note

${{{main}}}

\end{{document}}
";

pub const NOTE_TEMPLATE: &'static str = r"\documentclass[class=memoir, crop=false, oneside, 14pt]{standalone}

% document metadata
\author{ {{author}} }
\title{ {{title}} }
\date{ {{date}} }

\begin{document}

\end{document}
";
