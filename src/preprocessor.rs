use anyhow::Result;
use log::{error, trace, warn};
use mdbook::book::{Book, BookItem, Chapter};
use mdbook::errors::Error;
use mdbook::preprocess::{CmdPreprocessor, Preprocessor, PreprocessorContext};

use crate::svgbob::*;

/// Svgbob preprocessor for mdbook.
pub struct Bob;

impl Bob {
    pub fn new() -> Self {
        Self
    }

    pub fn handle_preprocessing(&self) -> Result<()> {
        use semver::{Version, VersionReq};
        use std::io::{stdin, stdout};

        let (ctx, book) = CmdPreprocessor::parse_input(stdin())?;
        let current = Version::parse(&ctx.mdbook_version)?;
        let built = VersionReq::parse(&format!("~{}", mdbook::MDBOOK_VERSION))?;

        if ctx.mdbook_version != mdbook::MDBOOK_VERSION && !built.matches(&current) {
            warn!(
                "The {} plugin was built against version {} of mdbook, \
				      but we're being called from version {}, so may be incompatible.",
                self.name(),
                mdbook::MDBOOK_VERSION,
                ctx.mdbook_version
            );
        }
        let processed_book = self.run(&ctx, book)?;
        serde_json::to_writer(stdout(), &processed_book)?;
        Ok(())
    }
}

impl Preprocessor for Bob {
    fn name(&self) -> &str {
        "svgbob"
    }
    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer != "not-supported"
    }

    fn run(&self, ctx: &PreprocessorContext, mut book: Book) -> Result<Book, Error> {
        let settings = ctx
            .config
            .get_preprocessor(self.name())
            .map(cfg_to_settings)
            .unwrap_or_default();

        book.for_each_mut(|item| {
            if let BookItem::Chapter(chapter) = item {
                let _ = process_code_blocks(chapter, &settings)
                    .map(|s| {
                        chapter.content = s;
                        trace!("chapter '{}' processed", &chapter.name);
                    })
                    .map_err(|err| {
                        error!("{}", err);
                    });
            }
        });
        Ok(book)
    }
}

/// Find code-blocks \`\`\`bob, produce svg and place it instead code.
fn process_code_blocks(
    chapter: &mut Chapter,
    settings: &Settings,
) -> Result<String, std::fmt::Error> {
    use pulldown_cmark::{CodeBlockKind, CowStr, Event, Parser, Tag};
    use pulldown_cmark_to_cmark::cmark;

    enum State {
        None,
        Open,
        Closing,
    }

    let mut state = State::None;
    let mut buf = String::with_capacity(chapter.content.len());
    let events = Parser::new(&chapter.content)
        .map(|e| {
            use CodeBlockKind::*;
            use CowStr::*;
            use Event::*;
            use State::*;
            use Tag::{CodeBlock, Paragraph};

            match (&e, &mut state) {
                (Start(CodeBlock(Fenced(Borrowed("bob")))), None) => {
                    state = Open;
                    Some(Start(Paragraph))
                }

                (Text(Borrowed(text)), Open) => {
                    state = Closing;
                    Some(Html(bob_handler(text, settings).into()))
                }

                (End(CodeBlock(Fenced(Borrowed("bob")))), Closing) => {
                    state = None;
                    Some(End(Paragraph))
                }
                _ => Some(e),
            }
        })
        .flatten();
    cmark(events, &mut buf).map(|_| buf)
}

#[cfg(test)]
mod tests {
    #[test]
    fn process_code_blocks() {
        use super::{process_code_blocks, Chapter, Settings};

        let settings = Settings::default();
        let mut chapter = Chapter::new(
            "test",
            "```bob\n-->\n```".to_owned(),
            ".",
            Vec::with_capacity(0),
        );
        let result = process_code_blocks(&mut chapter, &settings).unwrap();
        assert!(result.contains("<svg"));
        assert!(result.contains("<line"));
        assert!(result.contains("#arrow"));
    }
}
