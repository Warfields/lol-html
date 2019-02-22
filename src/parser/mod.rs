#[macro_use]
mod tag_name;

#[macro_use]
mod state_machine;

mod lexer;
mod outputs;
mod tag_scanner;
mod text_type;
mod tree_builder_simulator;

use self::lexer::Lexer;
use self::state_machine::{ParsingLoopTerminationReason, StateMachine};
use self::tag_scanner::TagScanner;
use self::tree_builder_simulator::{TreeBuilderFeedback, TreeBuilderSimulator};
use crate::base::Chunk;
use failure::Error;
use std::cell::RefCell;
use std::rc::Rc;

pub use self::lexer::LexemeSink;
pub use self::outputs::*;
pub use self::tag_name::TagName;
pub use self::tag_scanner::TagHintSink;
pub use self::text_type::*;

#[derive(Debug, Copy, Clone)]
pub enum ParserDirective {
    ScanForTags,
    Lex,
}

impl<S: LexemeSink> LexemeSink for Rc<RefCell<S>> {
    #[inline]
    fn handle_tag(&mut self, lexeme: &TagLexeme<'_>) -> ParserDirective {
        self.borrow_mut().handle_tag(lexeme)
    }

    #[inline]
    fn handle_non_tag_content(&mut self, lexeme: &NonTagContentLexeme<'_>) {
        self.borrow_mut().handle_non_tag_content(lexeme);
    }
}

impl<S: TagHintSink> TagHintSink for Rc<RefCell<S>> {
    #[inline]
    fn handle_tag_hint(&mut self, tag_hint: &TagHint<'_>) -> ParserDirective {
        self.borrow_mut().handle_tag_hint(tag_hint)
    }
}

pub trait ParserOutputSink: LexemeSink + TagHintSink {}

pub struct Parser<S: ParserOutputSink> {
    lexer: Lexer<Rc<RefCell<S>>>,
    tag_scanner: TagScanner<Rc<RefCell<S>>>,
    current_directive: ParserDirective,
}

// NOTE: dynamic dispatch can't be used for the StateMachine trait
// because it's not object-safe due to the usage of `Self` in function
// signatures, so we use this macro instead.
macro_rules! with_current_sm {
    ($self:tt, sm.$fn:ident($($args:tt)*) ) => {
        match $self.current_directive {
            ParserDirective::ScanForTags => $self.tag_scanner.$fn($($args)*),
            ParserDirective::Lex => $self.lexer.$fn($($args)*),
        }
    };
}

impl<S: ParserOutputSink> Parser<S> {
    pub fn new(output_sink: &Rc<RefCell<S>>, initial_directive: ParserDirective) -> Self {
        let tree_builder_simulator = Rc::new(RefCell::new(TreeBuilderSimulator::default()));

        Parser {
            lexer: Lexer::new(Rc::clone(output_sink), Rc::clone(&tree_builder_simulator)),
            tag_scanner: TagScanner::new(
                Rc::clone(output_sink),
                Rc::clone(&tree_builder_simulator),
            ),
            current_directive: initial_directive,
        }
    }

    pub fn parse(&mut self, input: &Chunk<'_>) -> Result<usize, Error> {
        let mut loop_termination_reason = with_current_sm!(self, sm.run_parsing_loop(input))?;

        loop {
            match loop_termination_reason {
                ParsingLoopTerminationReason::ParserDirectiveChange(new_directive, sm_bookmark) => {
                    self.current_directive = new_directive;

                    trace!(@continue_from_bookmark sm_bookmark, self.current_directive, input);

                    loop_termination_reason =
                        with_current_sm!(self, sm.continue_from_bookmark(input, sm_bookmark))?;
                }

                // NOTE: lexeme was required to get tree builder feedback for eager
                // parser. So we need to spin lexer and consume lexeme
                // for the tag, but without emitting it to consumers as they don't expect
                // lexemes at this point.
                ParsingLoopTerminationReason::LexemeRequiredForAdjustment(sm_bookmark) => {
                    self.current_directive = ParserDirective::Lex;

                    trace!(@continue_from_bookmark sm_bookmark, self.current_directive, input);

                    loop_termination_reason = self
                        .lexer
                        .silently_consume_current_tag_only(input, sm_bookmark)?;
                }

                ParsingLoopTerminationReason::EndOfInput { blocked_byte_count } => {
                    return Ok(blocked_byte_count);
                }
            }
        }
    }
}

#[cfg(feature = "testing_api")]
impl<S: ParserOutputSink> Parser<S> {
    pub fn switch_text_type(&mut self, text_type: TextType) {
        with_current_sm!(self, sm.switch_text_type(text_type));
    }

    pub fn set_last_start_tag_name_hash(&mut self, name_hash: Option<u64>) {
        with_current_sm!(self, sm.set_last_start_tag_name_hash(name_hash));
    }
}