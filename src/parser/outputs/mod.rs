mod lexeme;
mod tag_name_info;
mod token_outline;

pub use self::lexeme::*;
pub use self::tag_name_info::TagNameInfo;
pub use self::token_outline::*;

#[derive(Debug)]
pub enum TagHint<'i> {
    StartTag(TagNameInfo<'i>),
    EndTag(TagNameInfo<'i>),
}